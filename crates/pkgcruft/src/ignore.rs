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
    cache: DashMap<Utf8PathBuf, IndexMap<ReportKind, (ReportSet, bool)>>,
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
    fn load_data(
        &self,
        scope: Scope,
        relpath: Utf8PathBuf,
    ) -> IndexMap<ReportKind, (ReportSet, bool)> {
        let path = self.repo.path().join(relpath);
        if scope == Scope::Version {
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
    pub fn generate<'a, 'b>(
        &'a self,
        scope: &'b ReportScope,
    ) -> impl Iterator<Item = RefMut<'a, Utf8PathBuf, IndexMap<ReportKind, (ReportSet, bool)>>>
           + use<'a, 'b> {
        IgnorePaths::new(scope).map(move |(scope, relpath)| {
            self.cache
                .entry(relpath.clone())
                .or_insert_with(|| self.load_data(scope, relpath))
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
    pub fn unused(&self) -> IndexMap<Utf8PathBuf, IndexSet<ReportSet>> {
        let mut unused = IndexMap::new();
        for entry in &self.cache {
            let (path, map) = entry.pair();
            let values: IndexSet<_> = map
                .values()
                .filter_map(|(set, used)| if !used { Some(*set) } else { None })
                .collect();
            if !values.is_empty() {
                let path = if path.extension().is_some() {
                    path.clone()
                } else {
                    path.join(".pkgcruft-ignore")
                };
                unused.insert(path, values);
            }
        }
        unused.sort_keys();
        unused
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
            let (path, map) = entry.pair();

            // output path context
            if path == "" {
                writeln!(f, "{}", self.repo)?;
            } else if path.extension().is_some() {
                writeln!(f, "{path}")?;
            } else {
                writeln!(f, "{path}/*")?;
            }

            // output report sets
            let sets: IndexSet<_> = map.values().map(|(set, _)| set).collect();
            for set in sets {
                writeln!(f, "  {set}")?;
            }
        }

        Ok(())
    }
}

/// Iterator over relative paths for ignore data in a repo targeting a scope.
struct IgnorePaths<'a> {
    target: &'a ReportScope,
    scope: Option<Scope>,
}

impl<'a> IgnorePaths<'a> {
    fn new(target: &'a ReportScope) -> Self {
        Self {
            target,
            scope: Some(Scope::Repo),
        }
    }
}

impl Iterator for IgnorePaths<'_> {
    type Item = (Scope, Utf8PathBuf);

    fn next(&mut self) -> Option<Self::Item> {
        self.scope.map(|scope| {
            // construct the relative path to check for ignore files
            let relpath = match (scope, self.target) {
                (Scope::Category, ReportScope::Category(category)) => category.into(),
                (Scope::Category, ReportScope::Package(cpn)) => cpn.category().into(),
                (Scope::Category, ReportScope::Version(cpv, _)) => cpv.category().into(),
                (Scope::Package, ReportScope::Package(cpn)) => cpn.to_string().into(),
                (Scope::Package, ReportScope::Version(cpv, _)) => cpv.cpn().to_string().into(),
                (Scope::Version, ReportScope::Version(cpv, _)) => cpv.relpath(),
                _ => Default::default(),
            };

            // set the scope to the next lower level
            self.scope = match scope {
                Scope::Repo => Some(Scope::Category),
                Scope::Category => Some(Scope::Package),
                Scope::Package => Some(Scope::Version),
                Scope::Version => None,
            };

            (scope, relpath)
        })
    }
}
