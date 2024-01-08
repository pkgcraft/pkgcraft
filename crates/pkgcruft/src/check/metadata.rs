use crossbeam_channel::Sender;
use itertools::Itertools;
use pkgcraft::dep;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::pkg::ebuild::raw::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{PackageReport, Report, ReportKind};
use crate::source::SourceKind;
use crate::Error;

use super::{Check, CheckKind, CheckRun, Scope};

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::Metadata,
    source: SourceKind::EbuildPackageRaw,
    scope: Scope::Package,
    priority: -9999,
    reports: &[
        ReportKind::Package(PackageReport::InvalidDependency),
        ReportKind::Package(PackageReport::MissingMetadata),
        ReportKind::Package(PackageReport::SourcingError),
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
    fn run(&self, pkg: &Pkg<'a>, tx: &Sender<Report>) -> crate::Result<()> {
        use PackageReport::*;
        let mut success = true;

        match pkg.metadata_raw() {
            Ok(raw) => {
                for key in pkg.eapi().dep_keys() {
                    if let Some(val) = raw.get(key) {
                        if dep::parse::package_dependency_set(val, pkg.eapi()).is_err() {
                            success = false;
                            let report = InvalidDependency.report(pkg, key.to_string());
                            tx.send(report).unwrap();
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
                    let report = MissingMetadata.report(pkg, missing.iter().join(", "));
                    tx.send(report).unwrap();
                }
            }
            Err(InvalidPkg { id: _, err }) => {
                success = false;
                let report = SourcingError.report(pkg, err);
                tx.send(report).unwrap();
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