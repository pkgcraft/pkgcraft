use itertools::Itertools;
use once_cell::sync::Lazy;
use pkgcraft::dep;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::pkg::ebuild::metadata::Key;
use pkgcraft::pkg::ebuild::raw::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{
    Report, ReportKind,
    VersionReport::{InvalidDependencySet, MissingMetadata, SourcingError},
};
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun, EbuildRawPkgCheckKind};

pub(super) static CHECK: Lazy<Check> = Lazy::new(|| {
    Check::build(CheckKind::EbuildRawPkg(EbuildRawPkgCheckKind::Metadata))
        .source(SourceKind::EbuildRaw)
        .priority(-9999)
        .reports([
            ReportKind::Version(InvalidDependencySet),
            ReportKind::Version(MissingMetadata),
            ReportKind::Version(SourcingError),
        ])
});

#[derive(Debug)]
pub(crate) struct MetadataCheck<'a> {
    _repo: &'a Repo,
}

impl<'a> MetadataCheck<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { _repo: repo }
    }
}

impl<'a> CheckRun<&Pkg<'a>> for MetadataCheck<'a> {
    fn run(&self, pkg: &Pkg<'a>, reports: &mut Vec<Report>) {
        let eapi = pkg.eapi();

        match pkg.metadata_raw() {
            Ok(raw) => {
                // check for required metadata
                let missing: Vec<_> = eapi
                    .mandatory_keys()
                    .iter()
                    .filter(|k| raw.get(k).is_none())
                    .sorted()
                    .collect();

                if !missing.is_empty() {
                    reports.push(MissingMetadata.report(pkg, missing.iter().join(", ")));
                }

                // verify depset parsing
                // TODO: improve contextual relevance for depset parsing failures (issue #153)
                for key in eapi.dep_keys() {
                    if let Some(val) = raw.get(key) {
                        if let Err(e) = dep::parse::package_dependency_set(val, eapi) {
                            let msg = format!("{key}: {e}");
                            reports.push(InvalidDependencySet.report(pkg, msg));
                        }
                    }
                }

                if let Some(val) = raw.get(&Key::LICENSE) {
                    if let Err(e) = dep::parse::license_dependency_set(val) {
                        let msg = format!("{}: {e}", Key::LICENSE);
                        reports.push(InvalidDependencySet.report(pkg, msg));
                    }
                }

                if let Some(val) = raw.get(&Key::PROPERTIES) {
                    if let Err(e) = dep::parse::properties_dependency_set(val) {
                        let msg = format!("{}: {e}", Key::PROPERTIES);
                        reports.push(InvalidDependencySet.report(pkg, msg));
                    }
                }

                if let Some(val) = raw.get(&Key::REQUIRED_USE) {
                    if let Err(e) = dep::parse::required_use_dependency_set(val, eapi) {
                        let msg = format!("{}: {e}", Key::REQUIRED_USE);
                        reports.push(InvalidDependencySet.report(pkg, msg));
                    }
                }

                if let Some(val) = raw.get(&Key::RESTRICT) {
                    if let Err(e) = dep::parse::restrict_dependency_set(val) {
                        let msg = format!("{}: {e}", Key::RESTRICT);
                        reports.push(InvalidDependencySet.report(pkg, msg));
                    }
                }
            }
            Err(InvalidPkg { id: _, err }) => {
                reports.push(SourcingError.report(pkg, err));
            }
            // no other pkgcraft error types should occur
            Err(e) => panic!("MetadataCheck failed: {e}"),
        }
    }
}
