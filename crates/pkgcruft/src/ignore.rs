use std::{fmt, fs};

use camino::Utf8PathBuf;
use dashmap::{mapref::one::Ref, DashMap};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::restrict::Scope;
use rayon::prelude::*;

use crate::report::{Report, ReportKind, ReportScope, ReportSet};

pub struct Ignore {
    repo: EbuildRepo,
    cache: DashMap<Utf8PathBuf, IndexSet<ReportKind>>,
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

    /// Load ignore data from ebuild lines or files.
    fn load_data(&self, scope: Scope, relpath: Utf8PathBuf) -> IndexSet<ReportKind> {
        let path = self.repo.path().join(relpath);
        if scope == Scope::Version {
            // TODO: use BufRead to avoid loading the entire ebuild file?
            let mut ignore = IndexSet::new();
            for line in fs::read_to_string(path).unwrap_or_default().lines() {
                let line = line.trim();
                if let Some(data) = line.strip_prefix("# pkgcruft-ignore: ") {
                    ignore.extend(
                        data.split_whitespace()
                            .filter_map(|x| x.parse::<ReportSet>().ok())
                            .flat_map(|x| x.expand(&self.default, &self.supported)),
                    )
                } else if !line.is_empty() && !line.starts_with("#") {
                    break;
                }
            }
            ignore
        } else {
            fs::read_to_string(path.join(".pkgcruft-ignore"))
                .unwrap_or_default()
                .lines()
                .filter_map(|x| x.parse::<ReportSet>().ok())
                .flat_map(|x| x.expand(&self.default, &self.supported))
                .collect()
        }
    }

    /// Return an iterator of ignore cache entries for a scope.
    pub fn generate<'a, 'b>(
        &'a self,
        scope: &'b ReportScope,
    ) -> impl Iterator<Item = Ref<'a, Utf8PathBuf, IndexSet<ReportKind>>> + use<'a, 'b> {
        IgnorePaths::new(scope).map(move |(scope, relpath)| {
            self.cache
                .entry(relpath.clone())
                .or_insert_with(|| self.load_data(scope, relpath))
                .downgrade()
        })
    }

    /// Determine if a report is ignored via any relevant ignore settings.
    pub fn ignored(&self, report: &Report) -> bool {
        self.generate(report.scope())
            .any(|x| x.contains(&report.kind))
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
            let (path, kinds) = entry.pair();
            let path = if path == "" { "repo" } else { path.as_str() };
            let kinds = kinds.iter().join(", ");
            writeln!(f, "{path}: {kinds}")?;
        }

        Ok(())
    }
}

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
