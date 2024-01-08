use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use crossbeam_channel::Sender;
use indexmap::IndexSet;
use once_cell::sync::Lazy;
use pkgcraft::macros::cmp_not_equal;
use pkgcraft::pkg::ebuild;
use pkgcraft::repo::ebuild::Repo;
use strum::{AsRefStr, EnumIter, EnumString};

use crate::report::{Report, ReportKind};
use crate::source::SourceKind;

pub mod dependency;
pub mod dropped_keywords;
pub mod metadata;
pub mod unstable_only;

#[derive(
    AsRefStr, EnumIter, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub enum Scope {
    Package,
    PackageSet,
}

#[derive(
    AsRefStr, EnumIter, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub enum CheckKind {
    Dependency,
    DroppedKeywords,
    Metadata,
    UnstableOnly,
}

impl Borrow<str> for CheckKind {
    fn borrow(&self) -> &str {
        self.as_ref()
    }
}

impl std::fmt::Display for CheckKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

#[derive(Debug, Clone)]
pub(crate) enum CheckRunner<'a> {
    Dependency(dependency::DependencyCheck<'a>),
    DroppedKeywords(dropped_keywords::DroppedKeywordsCheck<'a>),
    Metadata(metadata::MetadataCheck<'a>),
    UnstableOnly(unstable_only::UnstableOnlyCheck<'a>),
}

// TODO: rework to leverage compile time checks for type mismatches between checks, source, and runners
impl<'a> CheckRun<ebuild::Pkg<'a>> for CheckRunner<'a> {
    fn run(&self, item: &ebuild::Pkg<'a>, tx: &Sender<Report>) -> crate::Result<()> {
        use CheckRunner::*;
        match self {
            Dependency(c) => c.run(item, tx),
            _ => panic!("check not valid for ebuild pkg runs"),
        }
    }
}

// TODO: rework to leverage compile time checks for type mismatches between checks, source, and runners
impl<'a> CheckRun<ebuild::raw::Pkg<'a>> for CheckRunner<'a> {
    fn run(&self, item: &ebuild::raw::Pkg<'a>, tx: &Sender<Report>) -> crate::Result<()> {
        use CheckRunner::*;
        match self {
            Metadata(c) => c.run(item, tx),
            _ => panic!("check not valid for raw ebuild pkg runs"),
        }
    }
}

// TODO: rework to leverage compile time checks for type mismatches between checks, source, and runners
impl<'a> CheckRun<Vec<ebuild::Pkg<'a>>> for CheckRunner<'a> {
    fn run(&self, item: &Vec<ebuild::Pkg<'a>>, tx: &Sender<Report>) -> crate::Result<()> {
        use CheckRunner::*;
        match self {
            DroppedKeywords(c) => c.run(item, tx),
            UnstableOnly(c) => c.run(item, tx),
            _ => panic!("check not valid for ebuild pkg set runs"),
        }
    }
}

/// Run a check for a given item sending back any generated reports.
pub(crate) trait CheckRun<T> {
    fn run(&self, item: &T, tx: &Sender<Report>) -> crate::Result<()>;
}

#[derive(Debug, Copy, Clone)]
pub struct Check {
    kind: CheckKind,
    source: SourceKind,
    scope: Scope,
    priority: i64,
    reports: &'static [ReportKind],
}

impl Check {
    pub fn kind(&self) -> CheckKind {
        self.kind
    }

    pub fn priority(&self) -> i64 {
        self.priority
    }

    pub fn source(&self) -> SourceKind {
        self.source
    }

    pub fn scope(&self) -> Scope {
        self.scope
    }

    pub fn reports(&self) -> &[ReportKind] {
        self.reports
    }

    pub(crate) fn to_runner(self, repo: &Repo) -> CheckRunner {
        use CheckKind::*;
        match self.kind {
            Dependency => CheckRunner::Dependency(dependency::DependencyCheck::new(repo)),
            DroppedKeywords => {
                CheckRunner::DroppedKeywords(dropped_keywords::DroppedKeywordsCheck::new(repo))
            }
            Metadata => CheckRunner::Metadata(metadata::MetadataCheck::new(repo)),
            UnstableOnly => CheckRunner::UnstableOnly(unstable_only::UnstableOnlyCheck::new(repo)),
        }
    }
}

impl PartialEq for Check {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Eq for Check {}

impl Hash for Check {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state)
    }
}

impl Borrow<str> for Check {
    fn borrow(&self) -> &str {
        self.kind.as_ref()
    }
}

impl Borrow<CheckKind> for Check {
    fn borrow(&self) -> &CheckKind {
        &self.kind
    }
}

impl Ord for Check {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_not_equal!(&self.priority, &other.priority);
        self.kind.cmp(&other.kind)
    }
}

impl PartialOrd for Check {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<&CheckKind> for &'static Check {
    fn from(kind: &CheckKind) -> Self {
        CHECKS.get(kind).expect("unregistered check: {kind}")
    }
}

pub static CHECKS: Lazy<IndexSet<Check>> = Lazy::new(|| {
    [dependency::CHECK, dropped_keywords::CHECK, metadata::CHECK, unstable_only::CHECK]
        .into_iter()
        .collect()
});
