use std::fs;

use camino::Utf8PathBuf;
use dashmap::DashMap;
use indexmap::IndexSet;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::restrict::Scope;

use crate::report::{Report, ReportKind, ReportScope, ReportSet};

pub struct Ignore {
    cache: DashMap<Utf8PathBuf, IndexSet<ReportKind>>,
    default: IndexSet<ReportKind>,
    supported: IndexSet<ReportKind>,
    repo: EbuildRepo,
}

impl Ignore {
    /// Create a new ignore cache for a repo.
    pub fn new(repo: EbuildRepo) -> Self {
        Self {
            default: ReportKind::defaults(&repo),
            supported: ReportKind::supported(&repo, Scope::Repo),
            cache: Default::default(),
            repo,
        }
    }

    /// Determine if a report is ignored via any relevant ignore files.
    pub fn ignored(&self, report: &Report) -> bool {
        IgnorePaths::new(report.scope()).any(|(scope, relpath)| {
            self.cache
                .entry(relpath.clone())
                .or_insert_with(|| {
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
                                        .flat_map(|x| {
                                            x.expand(&self.default, &self.supported)
                                        }),
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
                })
                .contains(&report.kind)
        })
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
