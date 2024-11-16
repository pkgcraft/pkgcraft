use itertools::Itertools;
use pkgcraft::dep::DependencySet;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::pkg::ebuild::metadata::Key;
use pkgcraft::pkg::ebuild::EbuildRawPkg;
use pkgcraft::pkg::Package;

use crate::bash::Tree;
use crate::report::ReportKind::{
    DependencyInvalid, LicenseInvalid, MetadataMissing, PropertiesInvalid, RequiredUseInvalid,
    RestrictInvalid, SourcingError,
};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildRawPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Metadata,
    scope: Scope::Version,
    source: SourceKind::EbuildRawPkg,
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

pub(super) fn create() -> impl EbuildRawPkgCheck {
    Check
}

struct Check;

super::register!(Check);

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, _tree: &Tree, filter: &mut ReportFilter) {
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
                    MetadataMissing
                        .version(pkg)
                        .message(missing.into_iter().join(", "))
                        .report(filter);
                }

                // verify depset parsing
                // TODO: improve contextual relevance for depset parsing failures (issue #153)
                for key in eapi.dep_keys() {
                    if let Some(val) = raw.get(key) {
                        if let Err(e) = DependencySet::package(val, eapi) {
                            DependencyInvalid
                                .version(pkg)
                                .message(format!("{key}: {e}"))
                                .report(filter);
                        }
                    }
                }

                if let Some(val) = raw.get(&Key::LICENSE) {
                    if let Err(e) = DependencySet::license(val) {
                        LicenseInvalid.version(pkg).message(e).report(filter);
                    }
                }

                if let Some(val) = raw.get(&Key::PROPERTIES) {
                    if let Err(e) = DependencySet::properties(val) {
                        PropertiesInvalid.version(pkg).message(e).report(filter);
                    }
                }

                if let Some(val) = raw.get(&Key::REQUIRED_USE) {
                    if let Err(e) = DependencySet::required_use(val) {
                        RequiredUseInvalid.version(pkg).message(e).report(filter);
                    }
                }

                if let Some(val) = raw.get(&Key::RESTRICT) {
                    if let Err(e) = DependencySet::restrict(val) {
                        RestrictInvalid.version(pkg).message(e).report(filter);
                    }
                }
            }
            Err(InvalidPkg { id: _, err }) => {
                SourcingError.version(pkg).message(err).report(filter);
            }
            // no other pkgcraft error types should occur
            Err(e) => panic!("MetadataCheck failed: {e}"),
        }
    }
}
