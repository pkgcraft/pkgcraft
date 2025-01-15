use std::{fmt, fs};

use camino::Utf8PathBuf;
use dashmap::{mapref::one::RefMut, DashMap};
use indexmap::{IndexMap, IndexSet};
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};
use pkgcraft::restrict::{Restrict, Scope};
use pkgcraft::traits::FilterLines;
use rayon::prelude::*;

use crate::report::{Report, ReportKind, ReportScope, ReportSet};

/// The cache of ignore data for an ebuild repo.
///
/// This is lazily and concurrently populated during scanning runs as reports are
/// generated.
pub struct Ignore {
    repo: EbuildRepo,
    cache: DashMap<ReportScope, IndexMap<ReportKind, (ReportSet, bool)>>,
    default: IndexSet<ReportKind>,
    supported: IndexSet<ReportKind>,
}

impl Ignore {
    /// Create a new ignore cache for a repo.
    pub fn new(repo: &EbuildRepo) -> Self {
        Self {
            repo: repo.clone(),
            cache: Default::default(),
            default: ReportKind::defaults(repo),
            supported: ReportKind::supported(repo, Scope::Repo),
        }
    }

    /// Parse the ignore data from a line.
    ///
    /// This supports comma-separated values with optional whitespace.
    fn parse_line<'a>(
        &'a self,
        data: &'a str,
    ) -> impl Iterator<Item = (ReportKind, (ReportSet, bool))> + 'a {
        data.split(',')
            .filter_map(|x| x.trim().parse::<ReportSet>().ok())
            .flat_map(move |set| {
                set.expand(&self.default, &self.supported)
                    .map(move |kind| (kind, (set, false)))
            })
    }

    /// Load ignore data from ebuild lines or files.
    fn load_data(&self, scope: ReportScope) -> IndexMap<ReportKind, (ReportSet, bool)> {
        let path = self.repo.path().join(scope_to_path(&scope));
        if matches!(scope, ReportScope::Version(..)) {
            // TODO: use BufRead to avoid loading the entire ebuild file?
            let mut ignore = IndexMap::new();
            for line in fs::read_to_string(path).unwrap_or_default().lines() {
                let line = line.trim();
                if let Some(data) = line.strip_prefix("# pkgcruft-ignore: ") {
                    ignore.extend(self.parse_line(data));
                } else if !line.is_empty() && !line.starts_with("#") {
                    break;
                }
            }
            ignore
        } else {
            fs::read_to_string(path.join(".pkgcruft-ignore"))
                .unwrap_or_default()
                .filter_lines()
                .flat_map(|(_, data)| self.parse_line(data))
                .collect()
        }
    }

    /// Return an iterator of ignore cache entries for a scope.
    ///
    /// This populates the cache in order of precedence for the scope, returning
    /// references to the generated entries. Note that all iterations generate their
    /// respective cache entry even ones lacking data so future matching lookups hit the
    /// cache rather than regenerating the data.
    pub(crate) fn generate<'a, 'b>(
        &'a self,
        scope: &'b ReportScope,
    ) -> impl Iterator<Item = RefMut<'a, ReportScope, IndexMap<ReportKind, (ReportSet, bool)>>>
           + use<'a, 'b> {
        IgnoreScopes::new(&self.repo, scope).map(move |scope| {
            self.cache
                .entry(scope.clone())
                .or_insert_with(|| self.load_data(scope))
        })
    }

    /// Determine if a report is ignored via any relevant ignore settings.
    ///
    /// For example, a version scope report will check for repo, category, package, and
    /// ebuild ignore data stopping at the first match.
    pub fn ignored(&self, report: &Report) -> bool {
        self.generate(report.scope()).any(|mut entry| {
            if let Some((_, used)) = entry.get_mut(&report.kind) {
                *used = true;
                true
            } else {
                false
            }
        })
    }

    /// Fully populate the cache for a restriction.
    pub fn populate(&self, restrict: &Restrict) {
        // TODO: replace with parallel Cpv iterator
        self.repo
            .iter_cpv_restrict(restrict)
            .collect::<Vec<_>>()
            .into_par_iter()
            .for_each(|cpv| {
                let scope = ReportScope::Version(cpv, None);
                self.generate(&scope).count();
            });
    }

    /// Return the mapping of unused ignore directives in the repo.
    pub fn unused(&self, scope: &ReportScope) -> Option<IndexSet<ReportSet>> {
        self.cache.get(scope).and_then(|entry| {
            let map = entry.value();
            let sets: IndexSet<_> = map
                .values()
                .filter_map(|(set, used)| if !used { Some(*set) } else { None })
                .collect();
            if !sets.is_empty() {
                Some(sets)
            } else {
                None
            }
        })
    }
}

impl fmt::Display for Ignore {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut entries: Vec<_> = self
            .cache
            .par_iter()
            .filter(|entry| !entry.value().is_empty())
            .collect();

        entries.sort_by(|a, b| a.key().cmp(b.key()));
        for entry in entries {
            let (scope, map) = entry.pair();
            writeln!(f, "{scope}")?;
            // output report sets
            let sets: IndexSet<_> = map.values().map(|(set, _)| set).collect();
            for set in sets {
                writeln!(f, "  {set}")?;
            }
        }

        Ok(())
    }
}

fn scope_to_path(scope: &ReportScope) -> Utf8PathBuf {
    match scope {
        ReportScope::Version(cpv, _) => cpv.relpath(),
        ReportScope::Package(cpn) => cpn.to_string().into(),
        ReportScope::Category(category) => category.into(),
        ReportScope::Repo(_) => Default::default(),
    }
}

/// Iterator over relevant scopes for ignore data in a repo targeting a scope.
struct IgnoreScopes<'a, 'b> {
    repo: &'a EbuildRepo,
    target: &'b ReportScope,
    scope: Option<Scope>,
}

impl<'a, 'b> IgnoreScopes<'a, 'b> {
    fn new(repo: &'a EbuildRepo, target: &'b ReportScope) -> Self {
        Self {
            repo,
            target,
            scope: Some(Scope::Repo),
        }
    }
}

impl Iterator for IgnoreScopes<'_, '_> {
    type Item = ReportScope;

    fn next(&mut self) -> Option<Self::Item> {
        self.scope.map(|scope| {
            // construct the relative path to check for ignore files
            let entry_scope = match (scope, self.target) {
                (Scope::Version, ReportScope::Version(..)) => self.target.clone(),
                (Scope::Package, ReportScope::Version(cpv, _)) => {
                    ReportScope::Package(cpv.cpn().clone())
                }
                (Scope::Package, ReportScope::Package(_)) => self.target.clone(),
                (Scope::Category, ReportScope::Category(_)) => self.target.clone(),
                (Scope::Category, ReportScope::Package(cpn)) => {
                    ReportScope::Category(cpn.category().into())
                }
                (Scope::Category, ReportScope::Version(cpv, _)) => {
                    ReportScope::Category(cpv.category().into())
                }
                _ => ReportScope::Repo(self.repo.to_string()),
            };

            // set the scope to the next lower level
            self.scope = match scope {
                Scope::Repo => Some(Scope::Category),
                Scope::Category => Some(Scope::Package),
                Scope::Package => Some(Scope::Version),
                Scope::Version => None,
            };

            entry_scope
        })
    }
}
