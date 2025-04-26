use std::{fmt, fs};

use dashmap::{DashMap, mapref::one::RefMut};
use indexmap::{IndexMap, IndexSet};
use itertools::Either;
use pkgcraft::repo::{EbuildRepo, PkgRepository};
use pkgcraft::restrict::{Restrict, Scope};
use pkgcraft::traits::FilterLines;
use rayon::prelude::*;
use tracing::warn;

use crate::report::{Report, ReportKind, ReportScope, ReportSet};
use crate::scan::ScannerRun;

/// Individual ignore directive relating to a ReportSet.
#[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy)]
pub(crate) struct IgnoreDirective {
    location: (usize, usize),
    set: ReportSet,
    used: bool,
    invert: bool,
}

impl IgnoreDirective {
    fn try_new(s: &str, location: (usize, usize)) -> crate::Result<Self> {
        let (s, invert) = s.strip_prefix('!').map_or((s, false), |x| (x, true));

        s.parse().map(|set| Self {
            location,
            set,
            used: false,
            invert,
        })
    }
}

impl fmt::Display for IgnoreDirective {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let invert = if self.invert { "!" } else { "" };
        write!(f, "{invert}{}", self.set)
    }
}

type CacheEntry = IndexMap<ReportKind, Vec<IgnoreDirective>>;

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
        let mut ignore: CacheEntry = IndexMap::new();

        // Parse ignore directives from a line.
        let mut parse_line = |csv: bool, data: &str, lineno: usize| {
            // support comma-separated values for ebuild files
            let values = if csv {
                Either::Left(
                    data.split(',')
                        .enumerate()
                        .map(|(i, s)| IgnoreDirective::try_new(s.trim(), (lineno, i))),
                )
            } else {
                Either::Right([IgnoreDirective::try_new(data.trim(), (lineno, 0))].into_iter())
            };

            for result in values {
                match result {
                    Ok(d) => {
                        for kind in d.set.expand(&self.default, &self.supported) {
                            ignore.entry(kind).or_default().push(d);
                        }
                    }
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

        let path = scope.to_abspath(&self.repo);
        if matches!(scope, ReportScope::Version(..)) {
            // TODO: use BufRead to avoid loading the entire ebuild file?
            for (i, line) in fs::read_to_string(path)
                .unwrap_or_default()
                .lines()
                .enumerate()
            {
                let line = line.trim();
                if let Some(data) = line.strip_prefix("# pkgcruft-ignore: ") {
                    parse_line(true, data, i + 1);
                } else if !line.is_empty() && !line.starts_with("#") {
                    break;
                }
            }
        } else {
            for (i, line) in fs::read_to_string(path.join(".pkgcruft-ignore"))
                .unwrap_or_default()
                .filter_lines()
            {
                parse_line(false, line, i);
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

    /// Determine if a report is ignored via relevant ignore directives.
    ///
    /// For example, a version scope report will check for repo, category, package, and
    /// ebuild directives.
    pub(crate) fn ignored(&self, report: &Report, run: &ScannerRun) -> bool {
        let mut ignored = false;

        for mut entry in self.generate(report.scope(), Some(run)) {
            if let Some(directives) = entry.get_mut(&report.kind) {
                for d in directives {
                    d.used = true;
                    ignored = !d.invert;
                }
            }
        }

        ignored
    }

    /// Return the set of unused ignore directives for a scope if it exists.
    pub fn unused(&self, scope: &ReportScope) -> Option<IndexSet<ReportSet>> {
        self.cache.get(scope).and_then(|entry| {
            let map = entry.value();
            let sets: IndexSet<_> = map
                .values()
                .flat_map(|directives| {
                    directives
                        .iter()
                        .filter_map(|d| if !d.used { Some(d.set) } else { None })
                })
                .collect();
            if !sets.is_empty() { Some(sets) } else { None }
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
            let mut directives: IndexSet<_> = map.values().flatten().collect();
            directives.sort_unstable();
            for d in directives {
                writeln!(f, "  {d}")?;
            }
        }

        Ok(())
    }
}

/// Iterator over relevant scopes for ignore data in a repo targeting a scope.
///
/// This iterates in reverse precedence order allowing more specific ignore entries to
/// override those at a larger scope. For example, package specific entries override repo
/// settings.
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
