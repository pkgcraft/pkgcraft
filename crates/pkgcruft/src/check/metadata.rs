use itertools::Itertools;
use pkgcraft::dep::DependencySet;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::pkg::ebuild::metadata::Key;
use pkgcraft::pkg::ebuild::raw::Pkg;
use pkgcraft::pkg::Package;

use crate::report::ReportKind::{
    DependencyInvalid, LicenseInvalid, MetadataMissing, PropertiesInvalid, RequiredUseInvalid,
    RestrictInvalid, SourcingError,
};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, RawVersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Metadata,
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

pub(super) fn create() -> impl RawVersionCheck {
    Check
}

struct Check;

super::register!(Check);

impl RawVersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
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
                    filter.report(MetadataMissing.version(pkg, message));
                }

                // verify depset parsing
                // TODO: improve contextual relevance for depset parsing failures (issue #153)
                for key in eapi.dep_keys() {
                    if let Some(val) = raw.get(key) {
                        if let Err(e) = DependencySet::package(val, eapi) {
                            let message = format!("{key}: {e}");
                            filter.report(DependencyInvalid.version(pkg, message));
                        }
                    }
                }

                if let Some(val) = raw.get(&Key::LICENSE) {
                    if let Err(e) = DependencySet::license(val) {
                        filter.report(LicenseInvalid.version(pkg, e));
                    }
                }

                if let Some(val) = raw.get(&Key::PROPERTIES) {
                    if let Err(e) = DependencySet::properties(val) {
                        filter.report(PropertiesInvalid.version(pkg, e));
                    }
                }

                if let Some(val) = raw.get(&Key::REQUIRED_USE) {
                    if let Err(e) = DependencySet::required_use(val) {
                        filter.report(RequiredUseInvalid.version(pkg, e));
                    }
                }

                if let Some(val) = raw.get(&Key::RESTRICT) {
                    if let Err(e) = DependencySet::restrict(val) {
                        filter.report(RestrictInvalid.version(pkg, e));
                    }
                }
            }
            Err(InvalidPkg { id: _, err }) => {
                filter.report(SourcingError.version(pkg, err));
            }
            // no other pkgcraft error types should occur
            Err(e) => panic!("MetadataCheck failed: {e}"),
        }
    }
}
