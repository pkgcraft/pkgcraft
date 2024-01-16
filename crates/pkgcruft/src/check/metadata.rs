use itertools::Itertools;
use pkgcraft::dep;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::pkg::ebuild::raw::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{Report, ReportKind, VersionReport};
use crate::scope::Scope;
use crate::source::SourceKind;
use crate::Error;

use super::{Check, CheckKind, CheckRun};

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::Metadata,
    source: SourceKind::EbuildPackageRaw,
    scope: Scope::Version,
    priority: -9999,
    reports: &[
        ReportKind::Version(VersionReport::InvalidDependency),
        ReportKind::Version(VersionReport::MissingMetadata),
        ReportKind::Version(VersionReport::SourcingError),
    ],
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
    fn run(&self, pkg: &Pkg<'a>, reports: &mut Vec<Report>) -> crate::Result<()> {
        use VersionReport::*;

        match pkg.metadata_raw() {
            Ok(raw) => {
                for key in pkg.eapi().dep_keys() {
                    if let Some(val) = raw.get(key) {
                        // TODO: add error output in report message once contextual relevance is
                        // fixed (issue #153)
                        if dep::parse::package_dependency_set(val, pkg.eapi()).is_err() {
                            reports.push(InvalidDependency.report(pkg, key.to_string()));
                        }
                    }
                }

                let missing: Vec<_> = pkg
                    .eapi()
                    .mandatory_keys()
                    .iter()
                    .filter(|k| raw.get(k).is_none())
                    .sorted()
                    .collect();

                if !missing.is_empty() {
                    reports.push(MissingMetadata.report(pkg, missing.iter().join(", ")));
                }
            }
            Err(InvalidPkg { id: _, err }) => {
                reports.push(SourcingError.report(pkg, err));
            }
            // no other pkgcraft error types should occur
            Err(e) => panic!("MetadataCheck failed: {e}"),
        }

        if reports.is_empty() {
            Ok(())
        } else {
            Err(Error::SkipRemainingChecks(CheckKind::Metadata))
        }
    }
}
