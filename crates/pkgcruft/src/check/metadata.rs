use crossbeam_channel::Sender;
use itertools::Itertools;
use pkgcraft::dep::{self, Version};
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::pkg::ebuild::metadata::Key;
use pkgcraft::pkg::ebuild::raw::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{Report, ReportKind};
use crate::source::SourceKind;
use crate::Error;

use super::{Check, CheckKind, CheckRun, Scope};

pub struct InvalidDependency {
    category: String,
    package: String,
    version: Version<String>,
    key: Key,
}

impl std::fmt::Display for InvalidDependency {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}/{}-{}: InvalidDependency: {}",
            self.category, self.package, self.version, self.key
        )
    }
}

impl From<InvalidDependency> for Report {
    fn from(value: InvalidDependency) -> Self {
        Self::InvalidDependency(value)
    }
}

pub struct SourcingError {
    category: String,
    package: String,
    version: Version<String>,
    error: String,
}

impl std::fmt::Display for SourcingError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}/{}-{}: SourcingError: {}",
            self.category, self.package, self.version, self.error
        )
    }
}

impl From<SourcingError> for Report {
    fn from(value: SourcingError) -> Self {
        Self::SourcingError(value)
    }
}

pub struct MissingMetadata {
    category: String,
    package: String,
    version: Version<String>,
    keys: Vec<Key>,
}

impl std::fmt::Display for MissingMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}/{}-{}: MissingMetadata: {}",
            self.category,
            self.package,
            self.version,
            self.keys.iter().join(", ")
        )
    }
}

impl From<MissingMetadata> for Report {
    fn from(value: MissingMetadata) -> Self {
        Self::MissingMetadata(value)
    }
}

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::Metadata,
    source: SourceKind::EbuildPackageRaw,
    scope: Scope::Package,
    priority: -9999,
    reports: &[ReportKind::InvalidDependency],
};

#[derive(Debug, Clone)]
pub(crate) struct MetadataCheck<'a> {
    _repo: &'a Repo,
}

impl<'a> MetadataCheck<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { _repo: repo }
    }
}

impl<'a> CheckRun<Pkg<'a>> for MetadataCheck<'_> {
    fn run(&self, pkg: &Pkg<'a>, tx: &Sender<Report>) -> crate::Result<()> {
        let mut success = true;

        match pkg.metadata_raw() {
            Ok(raw) => {
                for key in pkg.eapi().dep_keys() {
                    if let Some(val) = raw.get(key) {
                        if dep::parse::package_dependency_set(val, pkg.eapi()).is_err() {
                            success = false;
                            tx.send(
                                InvalidDependency {
                                    category: pkg.cpv().category().to_string(),
                                    package: pkg.cpv().package().to_string(),
                                    version: pkg.cpv().version().clone(),
                                    key: *key,
                                }
                                .into(),
                            )
                            .unwrap();
                        }
                    }
                }

                let missing: Vec<_> = pkg
                    .eapi()
                    .mandatory_keys()
                    .iter()
                    .filter(|k| raw.get(k).is_none())
                    .copied()
                    .sorted()
                    .collect();

                if !missing.is_empty() {
                    success = false;
                    tx.send(
                        MissingMetadata {
                            category: pkg.cpv().category().to_string(),
                            package: pkg.cpv().package().to_string(),
                            version: pkg.cpv().version().clone(),
                            keys: missing,
                        }
                        .into(),
                    )
                    .unwrap();
                }
            }
            Err(InvalidPkg { id: _, err }) => {
                success = false;
                tx.send(
                    SourcingError {
                        category: pkg.cpv().category().to_string(),
                        package: pkg.cpv().package().to_string(),
                        version: pkg.cpv().version().clone(),
                        error: err,
                    }
                    .into(),
                )
                .unwrap();
            }
            // no other pkgcraft error types should occur
            Err(e) => panic!("MetadataCheck failed: {e}"),
        }

        if success {
            Ok(())
        } else {
            Err(Error::SkipRemainingChecks(CheckKind::Metadata))
        }
    }
}
