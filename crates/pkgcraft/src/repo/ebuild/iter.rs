use std::{iter, mem};

use crate::dep::{Cpn, Cpv, Operator};
use crate::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use crate::repo::PkgRepository;
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{Restrict, Restriction};
use crate::traits::{
    Contains, ParallelMap, ParallelMapIter, ParallelMapOrdered, ParallelMapOrderedIter,
};

use super::EbuildRepo;

/// Ordered iterable of results from constructing ebuild packages.
pub struct Iter(IterRaw);

impl Iter {
    pub(super) fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        Self(IterRaw::new(repo, restrict))
    }
}

impl Iterator for Iter {
    type Item = crate::Result<EbuildPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|r| r.and_then(|raw_pkg| raw_pkg.try_into()))
    }
}

/// Unordered iterable of results from constructing ebuild packages.
///
/// This constructs packages in parallel and returns them as completed.
pub struct IterUnordered {
    iter: ParallelMapIter<crate::Result<EbuildPkg>>,
}

impl IterUnordered {
    pub(super) fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        let pkgs = IterRaw::new(repo, restrict);
        let func =
            move |result: crate::Result<EbuildRawPkg>| result.and_then(|pkg| pkg.try_into());
        Self {
            iter: pkgs.par_map(func).into_iter(),
        }
    }
}

impl Iterator for IterUnordered {
    type Item = crate::Result<EbuildPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Ordered iterable of results from constructing ebuild packages.
///
/// This constructs packages in parallel and returns them in repo order.
pub struct IterOrdered {
    iter: ParallelMapOrderedIter<crate::Result<EbuildPkg>>,
}

impl IterOrdered {
    pub(super) fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        let pkgs = IterRaw::new(repo, restrict);
        let func =
            move |result: crate::Result<EbuildRawPkg>| result.and_then(|pkg| pkg.try_into());
        Self {
            iter: pkgs.par_map_ordered(func).into_iter(),
        }
    }
}

impl Iterator for IterOrdered {
    type Item = crate::Result<EbuildPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Iterable of valid, raw ebuild packages.
pub struct IterRaw {
    iter: IterCpv,
    repo: EbuildRepo,
}

impl IterRaw {
    pub(super) fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        Self {
            iter: IterCpv::new(repo, restrict),
            repo: repo.clone(),
        }
    }
}

impl Iterator for IterRaw {
    type Item = crate::Result<EbuildRawPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .map(|cpv| EbuildRawPkg::try_new(cpv, &self.repo))
    }
}

/// Iterator of [`Cpn`] objects.
pub enum IterCpn {
    /// No matches
    Empty,

    /// Exact match
    Exact(std::iter::Once<Cpn>),

    /// Matches with package restriction
    Package {
        category: String,
        packages: indexmap::set::IntoIter<String>,
        pkg_restrict: Restrict,
    },

    /// Matches with category restriction
    Category {
        categories: std::vec::IntoIter<String>,
        package: String,
        repo: EbuildRepo,
    },

    /// Matches with custom restrictions
    Custom {
        categories: indexmap::set::IntoIter<String>,
        cat_packages: Option<(String, indexmap::set::IntoIter<String>)>,
        repo: EbuildRepo,
        cat_restrict: Restrict,
        pkg_restrict: Restrict,
    },
}

impl IterCpn {
    /// Create a new IterCpn iterator.
    pub(super) fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        use DepRestrict::{Category, Package};
        use StrRestrict::Equal;
        let mut cat_restricts = vec![];
        let mut pkg_restricts = vec![];

        // extract matching restrictions for optimized iteration
        match restrict {
            Some(Restrict::False) => return Self::Empty,
            Some(restrict) => {
                let mut match_restrict = |restrict: &Restrict| match restrict {
                    Restrict::Dep(Category(r)) => cat_restricts.push(r.clone()),
                    Restrict::Dep(Package(r)) => pkg_restricts.push(r.clone()),
                    _ => (),
                };

                if let Restrict::And(vals) = restrict {
                    vals.iter().for_each(|x| match_restrict(x));
                } else {
                    match_restrict(restrict);
                }
            }
            _ => (),
        }

        match (&mut *cat_restricts, &mut *pkg_restricts) {
            ([Equal(cat)], [Equal(pn)]) => {
                let cat = mem::take(cat);
                let pn = mem::take(pn);
                Cpn::try_from((cat, pn))
                    .ok()
                    .filter(|cpn| repo.contains(cpn))
                    .map(|cpn| Self::Exact(iter::once(cpn)))
                    .unwrap_or(Self::Empty)
            }
            ([Equal(cat)], _) => {
                let category = mem::take(cat);
                let packages = repo.packages(&category).into_iter();
                let pkg_restrict = Restrict::and(pkg_restricts);
                Self::Package {
                    category,
                    packages,
                    pkg_restrict,
                }
            }
            (_, [Equal(pn)]) => {
                let package = mem::take(pn);
                let restrict = Restrict::and(cat_restricts);
                let categories: Vec<_> = repo
                    .categories()
                    .into_iter()
                    .filter(|cat| restrict.matches(cat))
                    .collect();
                Self::Category {
                    categories: categories.into_iter(),
                    package,
                    repo: repo.clone(),
                }
            }
            _ => {
                let cat_restrict = Restrict::and(cat_restricts);
                let pkg_restrict = Restrict::and(pkg_restricts);
                Self::Custom {
                    categories: repo.categories().into_iter(),
                    cat_packages: None,
                    repo: repo.clone(),
                    cat_restrict,
                    pkg_restrict,
                }
            }
        }
    }
}

impl Iterator for IterCpn {
    type Item = Cpn;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Exact(iter) => iter.next(),
            Self::Package {
                category,
                packages,
                pkg_restrict,
            } => packages.find(|pn| pkg_restrict.matches(pn)).map(|pn| Cpn {
                category: category.clone(),
                package: pn,
            }),
            Self::Category { categories, package, repo } => categories
                .map(|category| Cpn {
                    category,
                    package: package.clone(),
                })
                .find(|cpn| repo.contains(cpn)),
            Self::Custom {
                categories,
                cat_packages,
                repo,
                cat_restrict,
                pkg_restrict,
            } => loop {
                // determine which category to iterate through
                let (category, packages) = match cat_packages {
                    Some(value) => value,
                    None => {
                        let category = categories.find(|cat| cat_restrict.matches(cat))?;
                        let set = repo.packages(&category);
                        cat_packages.insert((category, set.into_iter()))
                    }
                };

                // look for matching packages in the selected category
                if let Some(package) = packages.find(|pn| pkg_restrict.matches(pn)) {
                    return Some(Cpn {
                        category: category.clone(),
                        package,
                    });
                }

                // reset category packages iterator
                cat_packages.take();
            },
        }
    }
}

/// Iterator of [`Cpv`] objects.
#[allow(clippy::large_enum_variant)]
pub enum IterCpv {
    /// No matches
    Empty,

    /// Exact match
    Exact(std::iter::Once<Cpv>),

    /// Matches with version restriction
    Version {
        iter: std::vec::IntoIter<Cpv>,
        ver_restrict: Restrict,
    },
}

impl IterCpv {
    /// Create a new IterCpv iterator.
    pub(super) fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        use DepRestrict::{Category, Package, Version};
        use StrRestrict::Equal;
        let mut cat_restricts = vec![];
        let mut pkg_restricts = vec![];
        let mut ver_restricts = vec![];

        // extract matching restrictions for optimized iteration
        match restrict {
            Some(Restrict::False) => return Self::Empty,
            Some(restrict) => {
                let mut match_restrict = |restrict: &Restrict| match restrict {
                    Restrict::Dep(r @ Category(_)) => cat_restricts.push(r.clone()),
                    Restrict::Dep(r @ Package(_)) => pkg_restricts.push(r.clone()),
                    Restrict::Dep(r @ Version(_)) => ver_restricts.push(r.clone()),
                    _ => (),
                };

                if let Restrict::And(vals) = restrict {
                    vals.iter().for_each(|x| match_restrict(x));
                } else {
                    match_restrict(restrict);
                }
            }
            _ => (),
        }

        match (&mut *cat_restricts, &mut *pkg_restricts, &mut *ver_restricts) {
            ([Category(Equal(cat))], [Package(Equal(pn))], [Version(Some(ver))])
                if ver.op().is_none() || ver.op() == Some(Operator::Equal) =>
            {
                Cpv::try_from((cat, pn, ver.without_op()))
                    .ok()
                    .filter(|cpv| repo.contains(cpv))
                    .map(|cpv| Self::Exact(iter::once(cpv)))
                    .unwrap_or(Self::Empty)
            }
            ([Category(Equal(cat))], [Package(Equal(pn))], _) => {
                let ver_restrict = Restrict::and(ver_restricts);
                let cpvs = repo
                    .cpvs_from_package(cat, pn)
                    .filter_map(Result::ok)
                    .collect::<Vec<_>>();
                Self::Version {
                    iter: cpvs.into_iter(),
                    ver_restrict,
                }
            }
            ([], [Package(Equal(pn))], _) => {
                let pn = mem::take(pn);
                let ver_restrict = Restrict::and(ver_restricts);
                let cpvs = repo
                    .categories()
                    .into_iter()
                    .flat_map(move |cat| repo.cpvs_from_package(&cat, &pn))
                    .filter_map(Result::ok)
                    .collect::<Vec<_>>();
                Self::Version {
                    iter: cpvs.into_iter(),
                    ver_restrict,
                }
            }
            _ => {
                let restricts: Vec<_> =
                    cat_restricts.into_iter().chain(pkg_restricts).collect();
                let cpn_restrict = Restrict::and(restricts);
                let ver_restrict = Restrict::and(ver_restricts);

                let cpvs = repo
                    .iter_cpn_restrict(cpn_restrict)
                    .flat_map(|cpn| repo.cpvs_from_package(&cpn.category, &cpn.package))
                    .filter_map(Result::ok)
                    .collect::<Vec<_>>();

                Self::Version {
                    iter: cpvs.into_iter(),
                    ver_restrict,
                }
            }
        }
    }
}

impl Iterator for IterCpv {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Exact(iter) => iter.next(),
            Self::Version { iter, ver_restrict } => iter.find(|cpv| ver_restrict.matches(cpv)),
        }
    }
}

/// Iterable of valid ebuild packages matching a given restriction.
pub struct IterRestrict {
    iter: Iter,
    restrict: Restrict,
}

impl IterRestrict {
    pub(super) fn new<R: Into<Restrict>>(repo: &EbuildRepo, value: R) -> Self {
        let restrict = value.into();
        let iter = Iter::new(repo, Some(&restrict));
        Self { iter, restrict }
    }
}

impl Iterator for IterRestrict {
    type Item = crate::Result<EbuildPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find_map(|r| match r {
            Ok(pkg) if self.restrict.matches(&pkg) => Some(Ok(pkg)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
    }
}

/// Ordered iterable of results from constructing ebuild packages matching a given
/// restriction.
///
/// This constructs packages in parallel and returns them in repo order.
pub struct IterRestrictOrdered {
    iter: IterOrdered,
    restrict: Restrict,
}

impl IterRestrictOrdered {
    pub(super) fn new<R: Into<Restrict>>(repo: &EbuildRepo, value: R) -> Self {
        let restrict = value.into();
        let iter = IterOrdered::new(repo, Some(&restrict));
        Self { iter, restrict }
    }
}

impl Iterator for IterRestrictOrdered {
    type Item = crate::Result<EbuildPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find_map(|r| match r {
            Ok(pkg) if self.restrict.matches(&pkg) => Some(Ok(pkg)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
    }
}

/// Iterable of [`Cpn`] objects matching a given restriction.
pub struct IterCpnRestrict {
    iter: IterCpn,
    restrict: Restrict,
}

impl IterCpnRestrict {
    pub(super) fn new<R: Into<Restrict>>(repo: &EbuildRepo, value: R) -> Self {
        let restrict = value.into();
        // TODO: Consider passing a mutable restriction to avoid re-running category and
        // package restrictions.
        let iter = IterCpn::new(repo, Some(&restrict));
        Self { iter, restrict }
    }
}

impl Iterator for IterCpnRestrict {
    type Item = Cpn;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|cpn| self.restrict.matches(cpn))
    }
}

/// Iterable of [`Cpv`] objects matching a given restriction.
pub struct IterCpvRestrict {
    iter: IterCpv,
    restrict: Restrict,
}

impl IterCpvRestrict {
    pub(super) fn new<R: Into<Restrict>>(repo: &EbuildRepo, value: R) -> Self {
        let restrict = value.into();
        // TODO: Consider passing a mutable restriction to avoid re-running category,
        // package, and version restrictions.
        let iter = IterCpv::new(repo, Some(&restrict));
        Self { iter, restrict }
    }
}

impl Iterator for IterCpvRestrict {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|cpv| self.restrict.matches(cpv))
    }
}

/// Iterable of valid, raw ebuild packages matching a given restriction.
pub struct IterRawRestrict {
    iter: IterRaw,
    restrict: Restrict,
}

impl IterRawRestrict {
    pub(super) fn new<R: Into<Restrict>>(repo: &EbuildRepo, value: R) -> Self {
        let restrict = value.into();
        let iter = IterRaw::new(repo, Some(&restrict));
        Self { iter, restrict }
    }
}

impl Iterator for IterRawRestrict {
    type Item = crate::Result<EbuildRawPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find_map(|r| match r {
            Ok(pkg) if self.restrict.matches(&pkg) => Some(Ok(pkg)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
    }
}

/// Ordered iterable of results from constructing raw packages.
///
/// This constructs packages in parallel and returns them in repo order.
pub struct IterRawOrdered {
    iter: ParallelMapOrderedIter<crate::Result<EbuildRawPkg>>,
}

impl IterRawOrdered {
    pub(super) fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        let cpvs = IterCpv::new(repo, restrict);
        let repo = repo.clone();
        let func = move |cpv: Cpv| repo.get_pkg_raw(cpv);
        Self {
            iter: cpvs.par_map_ordered(func).into_iter(),
        }
    }
}

impl Iterator for IterRawOrdered {
    type Item = crate::Result<EbuildRawPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Ordered iterable of results from constructing raw packages matching a given
/// restriction.
///
/// This constructs packages in parallel and returns them in repo order.
pub struct IterRawRestrictOrdered {
    iter: IterRawOrdered,
    restrict: Restrict,
}

impl IterRawRestrictOrdered {
    pub(super) fn new<R: Into<Restrict>>(repo: &EbuildRepo, value: R) -> Self {
        let restrict = value.into();
        let iter = IterRawOrdered::new(repo, Some(&restrict));
        Self { iter, restrict }
    }
}

impl Iterator for IterRawRestrictOrdered {
    type Item = crate::Result<EbuildRawPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find_map(|r| match r {
            Ok(pkg) if self.restrict.matches(&pkg) => Some(Ok(pkg)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
    }
}
