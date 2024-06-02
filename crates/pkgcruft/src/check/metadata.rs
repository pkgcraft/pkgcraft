use itertools::Itertools;
use pkgcraft::dep;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::pkg::ebuild::metadata::Key;
use pkgcraft::pkg::ebuild::raw::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{
    Report,
    ReportKind::{
        DependencyInvalid, LicenseInvalid, MetadataMissing, PropertiesInvalid, RequiredUseInvalid,
        RestrictInvalid, SourcingError,
    },
};
use crate::scope::Scope;
use crate::source::SourceKind;

pub(super) static CHECK: super::CheckInfo = super::CheckInfo {
    scope: Scope::Version,
    source: SourceKind::EbuildRaw,
    reports: &[
        DependencyInvalid,
        LicenseInvalid,
        PropertiesInvalid,
        RequiredUseInvalid,
        RestrictInvalid,
        MetadataMissing,
        SourcingError,
    ],
    context: &[],
    priority: -9999,
};

#[derive(Debug)]
pub(crate) struct Check<'a> {
    _repo: &'a Repo,
}

impl<'a> Check<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { _repo: repo }
    }
}

impl<'a> super::CheckRun<&Pkg<'a>> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkg: &Pkg<'a>, mut report: F) {
        let eapi = pkg.eapi();

        match pkg.metadata_raw() {
            Ok(raw) => {
                // check for required metadata
                let missing = eapi
                    .mandatory_keys()
                    .iter()
                    .filter(|k| raw.get(k).is_none())
                    .sorted()
                    .collect::<Vec<_>>();

                if !missing.is_empty() {
                    let message = missing.into_iter().join(", ");
                    report(MetadataMissing.version(pkg, message));
                }

                // verify depset parsing
                // TODO: improve contextual relevance for depset parsing failures (issue #153)
                for key in eapi.dep_keys() {
                    if let Some(val) = raw.get(key) {
                        if let Err(e) = dep::parse::package_dependency_set(val, eapi) {
                            let message = format!("{key}: {e}");
                            report(DependencyInvalid.version(pkg, message));
                        }
                    }
                }

                if let Some(val) = raw.get(&Key::LICENSE) {
                    if let Err(e) = dep::parse::license_dependency_set(val) {
                        report(LicenseInvalid.version(pkg, e));
                    }
                }

                if let Some(val) = raw.get(&Key::PROPERTIES) {
                    if let Err(e) = dep::parse::properties_dependency_set(val) {
                        report(PropertiesInvalid.version(pkg, e));
                    }
                }

                if let Some(val) = raw.get(&Key::REQUIRED_USE) {
                    if let Err(e) = dep::parse::required_use_dependency_set(val) {
                        report(RequiredUseInvalid.version(pkg, e));
                    }
                }

                if let Some(val) = raw.get(&Key::RESTRICT) {
                    if let Err(e) = dep::parse::restrict_dependency_set(val) {
                        report(RestrictInvalid.version(pkg, e));
                    }
                }
            }
            Err(InvalidPkg { id: _, err }) => {
                report(SourcingError.version(pkg, err));
            }
            // no other pkgcraft error types should occur
            Err(e) => panic!("MetadataCheck failed: {e}"),
        }
    }
}
