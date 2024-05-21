use itertools::Itertools;
use once_cell::sync::Lazy;
use pkgcraft::dep;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::pkg::ebuild::metadata::Key;
use pkgcraft::pkg::ebuild::raw::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{
    Report,
    ReportKind::{InvalidDependencySet, MissingMetadata, SourcingError},
};
use crate::source::SourceKind;

use super::{CheckBuilder, CheckKind, CheckRun};

pub(super) static CHECK: Lazy<super::Check> = Lazy::new(|| {
    CheckBuilder::new(CheckKind::Metadata)
        .source(SourceKind::EbuildRaw)
        .priority(-9999)
        .reports([InvalidDependencySet, MissingMetadata, SourcingError])
});

#[derive(Debug)]
pub(crate) struct Check<'a> {
    _repo: &'a Repo,
}

impl<'a> Check<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { _repo: repo }
    }
}

impl<'a> CheckRun<&Pkg<'a>> for Check<'a> {
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
                    let message = missing.iter().join(", ");
                    reports.push(MissingMetadata.version(pkg, message));
                }

                // verify depset parsing
                // TODO: improve contextual relevance for depset parsing failures (issue #153)
                for key in eapi.dep_keys() {
                    if let Some(val) = raw.get(key) {
                        if let Err(e) = dep::parse::package_dependency_set(val, eapi) {
                            let message = format!("{key}: {e}");
                            reports.push(InvalidDependencySet.version(pkg, message));
                        }
                    }
                }

                if let Some(val) = raw.get(&Key::LICENSE) {
                    if let Err(e) = dep::parse::license_dependency_set(val) {
                        let message = format!("{}: {e}", Key::LICENSE);
                        reports.push(InvalidDependencySet.version(pkg, message));
                    }
                }

                if let Some(val) = raw.get(&Key::PROPERTIES) {
                    if let Err(e) = dep::parse::properties_dependency_set(val) {
                        let message = format!("{}: {e}", Key::PROPERTIES);
                        reports.push(InvalidDependencySet.version(pkg, message));
                    }
                }

                if let Some(val) = raw.get(&Key::REQUIRED_USE) {
                    if let Err(e) = dep::parse::required_use_dependency_set(val, eapi) {
                        let message = format!("{}: {e}", Key::REQUIRED_USE);
                        reports.push(InvalidDependencySet.version(pkg, message));
                    }
                }

                if let Some(val) = raw.get(&Key::RESTRICT) {
                    if let Err(e) = dep::parse::restrict_dependency_set(val) {
                        let message = format!("{}: {e}", Key::RESTRICT);
                        reports.push(InvalidDependencySet.version(pkg, message));
                    }
                }
            }
            Err(InvalidPkg { id: _, err }) => {
                reports.push(SourcingError.version(pkg, err));
            }
            // no other pkgcraft error types should occur
            Err(e) => panic!("MetadataCheck failed: {e}"),
        }
    }
}
