use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, File};
use std::io::Write;

use camino::Utf8Path;
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use ordermap::OrderSet;
use rayon::prelude::*;

use crate::dep::{ConditionalFlatten, Cpn, Cpv, Dep, UseDep};
use crate::macros::build_path;
use crate::pkg::ebuild::metadata::Key;
use crate::pkg::Package;
use crate::traits::LogErrors;
use crate::Error;

use super::EbuildRepo;

/// Reverse dependency entry for the RevDepCache.
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct RevDep {
    cpv: Cpv,
    use_deps: OrderSet<UseDep>,
    dep: Dep,
}

impl RevDep {
    /// Return the Cpv for the reverse dependency.
    pub fn cpv(&self) -> &Cpv {
        &self.cpv
    }

    /// Return the package dependency for the reverse dependency.
    pub fn dep(&self) -> &Dep {
        &self.dep
    }
}

/// QA cache format for reverse dependency.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct QaRevDep<'a> {
    cpv: &'a Cpv,
    blocker: bool,
}

impl<'a> From<&'a RevDep> for QaRevDep<'a> {
    fn from(value: &'a RevDep) -> Self {
        Self {
            cpv: &value.cpv,
            blocker: value.dep.blocker().is_some(),
        }
    }
}

impl fmt::Display for QaRevDep<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.blocker {
            write!(f, "[B]")?;
        }
        write!(f, "{}", self.cpv)
    }
}

/// Cache of reverse dependencies for an ebuild repo.
#[derive(Debug, Default)]
pub struct RevDepCache(HashMap<Cpn, HashMap<RevDep, HashSet<Key>>>);

impl RevDepCache {
    /// Create a reverse dependencies cache from an ebuild repo.
    pub fn from_repo(repo: &EbuildRepo, ignore: bool) -> crate::Result<Self> {
        let mut cache = Self::default();

        // TODO: build cache in parallel
        let mut iter = repo.iter_unordered().log_errors(ignore);
        for pkg in &mut iter {
            for key in pkg.eapi().dep_keys().iter().copied() {
                for (mut use_deps, dep) in
                    pkg.dependencies([key]).into_iter_conditional_flatten()
                {
                    use_deps.sort();
                    cache
                        .0
                        .entry(dep.cpn.clone())
                        .or_default()
                        .entry(RevDep {
                            cpv: pkg.cpv().clone(),
                            use_deps: use_deps.into_iter().collect(),
                            dep: dep.clone(),
                        })
                        .or_default()
                        .insert(key);
                }
            }
        }

        if iter.failed() {
            Err(Error::InvalidValue("metadata failures occurred".to_string()))
        } else {
            Ok(cache)
        }
    }

    /// Get the reverse dependencies for a Cpn.
    pub fn get(&self, cpn: &Cpn) -> Option<&HashMap<RevDep, HashSet<Key>>> {
        self.0.get(cpn)
    }

    /// Serialize the cache to a directory using qa reports format.
    pub fn serialize_to_qa<P: AsRef<Utf8Path>>(&self, path: P) -> crate::Result<()> {
        let dir = path.as_ref().join("revdeps");

        // convert cache into qa reports compatible mapping
        let mut mapping: HashMap<_, HashMap<_, IndexMap<QaRevDep, IndexSet<_>>>> =
            HashMap::new();
        for (cpn, revdeps) in &self.0 {
            for (revdep, keys) in revdeps {
                for key in keys {
                    mapping
                        .entry(key.as_ref().to_lowercase())
                        .or_default()
                        .entry(cpn)
                        .or_default()
                        .entry(revdep.into())
                        .or_default()
                        .extend(&revdep.use_deps);
                }
            }
        }

        // return the prefix for the USE dependency
        let enabled = |use_dep: &UseDep| -> &str {
            if use_dep.enabled() {
                ""
            } else {
                "!"
            }
        };

        // write entries to disk in the expected file layout and format
        for (key, revdeps) in mapping {
            revdeps.into_par_iter().try_for_each(
                |(cpn, mut revdeps)| -> crate::Result<()> {
                    let path = build_path!(&dir, &key, cpn.category(), cpn.package());
                    fs::create_dir_all(path.parent().unwrap())?;
                    let mut f = File::create(path)?;
                    revdeps.sort_keys();
                    for (revdep, use_deps) in revdeps {
                        let flags = if !use_deps.is_empty() {
                            format!(
                                ":{}",
                                use_deps
                                    .iter()
                                    .map(|x| format!("{}{}", enabled(x), x.flag()))
                                    .join("+")
                            )
                        } else {
                            Default::default()
                        };
                        writeln!(f, "{revdep}{flags}")?;
                    }
                    f.flush()?;
                    Ok(())
                },
            )?;
        }

        Ok(())
    }
}
