use std::{fmt, fs};

use camino::Utf8PathBuf;
use dashmap::{mapref::one::RefMut, DashMap};
use indexmap::{IndexMap, IndexSet};
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};
use pkgcraft::restrict::{Restrict, Scope};
use pkgcraft::traits::FilterLines;
use rayon::prelude::*;
use tracing::warn;

use crate::report::{Report, ReportKind, ReportScope, ReportSet};
use crate::scan::ScannerRun;

type CacheEntry = IndexMap<ReportKind, (ReportSet, bool)>;

/// The cache of ignore data for an ebuild repo.
///
/// This is lazily and concurrently populated during scanning runs as reports are
/// generated.
pub struct Ignore {
    repo: EbuildRepo,
    cache: DashMap<ReportScope, CacheEntry>,
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

    /// Fully populate the cache for a restriction.
    pub fn populate(self, restrict: &Restrict) -> Self {
        // TODO: replace with parallel Cpv iterator
        self.repo
            .iter_cpv_restrict(restrict)
            .collect::<Vec<_>>()
            .into_par_iter()
            .for_each(|cpv| {
                let scope = ReportScope::Version(cpv, None);
                self.generate(&scope, None).count();
            });
        self
    }

    /// Load ignore data from ebuild lines or files.
    fn load_data(&self, scope: &ReportScope, run: Option<&ScannerRun>) -> CacheEntry {
        let relpath = scope_to_path(scope);
        let mut ignore = IndexMap::new();

        // Parse ignore directives from a line.
        //
        // This supports comma-separated values with optional whitespace.
        let mut parse_line = |data: &str, lineno: usize| {
            for result in data.split(',').map(|s| s.trim().parse::<ReportSet>()) {
                match result {
                    Ok(set) => ignore.extend(
                        set.expand(&self.default, &self.supported)
                            .map(move |kind| (kind, (set, false))),
                    ),
                    Err(e) => {
                        if let Some(run) = run {
                            ReportKind::IgnoreInvalid
                                .in_scope(scope.clone())
                                .message(e)
                                .report(run);
                        } else {
                            warn!("{scope}: invalid ignore directive: line {lineno}: {e}");
                        }
                    }
                }
            }
        };

        let abspath = self.repo.path().join(&relpath);
        if matches!(scope, ReportScope::Version(..)) {
            // TODO: use BufRead to avoid loading the entire ebuild file?
            for (i, line) in fs::read_to_string(abspath)
                .unwrap_or_default()
                .lines()
                .enumerate()
            {
                let line = line.trim();
                if let Some(data) = line.strip_prefix("# pkgcruft-ignore: ") {
                    parse_line(data, i + 1);
                } else if !line.is_empty() && !line.starts_with("#") {
                    break;
                }
            }
        } else {
            for (i, line) in fs::read_to_string(abspath.join(".pkgcruft-ignore"))
                .unwrap_or_default()
                .filter_lines()
            {
                parse_line(line, i);
            }
        }

        ignore
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
        run: Option<&'a ScannerRun>,
    ) -> impl Iterator<Item = RefMut<'a, ReportScope, CacheEntry>> + use<'a, 'b> {
        IgnoreScopes::new(&self.repo, scope).map(move |scope| {
            self.cache
                .entry(scope.clone())
                .or_insert_with(|| self.load_data(&scope, run))
        })
    }

    /// Determine if a report is ignored via any relevant ignore settings.
    ///
    /// For example, a version scope report will check for repo, category, package, and
    /// ebuild ignore data stopping at the first match.
    pub(crate) fn ignored(&self, report: &Report, run: &ScannerRun) -> bool {
        self.generate(report.scope(), Some(run)).any(|mut entry| {
            entry
                .get_mut(&report.kind)
                .map(|(_, used)| *used = true)
                .is_some()
        })
    }

    /// Return the set of unused ignore directives for a scope if it exists.
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

/// Convert scope to ignore file path.
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
