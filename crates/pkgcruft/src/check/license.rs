use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::{ebuild::Pkg, Package};
use pkgcraft::repo::ebuild::Repo;

use crate::report::ReportKind::{LicenseDeprecated, LicenseMissing, LicenseUnneeded};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, VersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::License,
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[LicenseDeprecated, LicenseMissing, LicenseUnneeded],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static Repo) -> impl VersionCheck {
    Check {
        deprecated: repo
            .license_groups()
            .get("DEPRECATED")
            .map(|x| x.iter().collect())
            .unwrap_or_default(),
        unlicensed_categories: ["acct-group", "acct-user", "virtual"]
            .iter()
            .map(|x| x.to_string())
            .collect(),
    }
}

struct Check {
    deprecated: IndexSet<&'static String>,
    unlicensed_categories: IndexSet<String>,
}

super::register!(Check);

impl VersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        let licenses: IndexSet<_> = pkg.license().iter_flatten().collect();
        if licenses.is_empty() {
            if !self.unlicensed_categories.contains(pkg.category()) {
                filter.report(LicenseMissing.version(pkg, ""));
            }
        } else if self.unlicensed_categories.contains(pkg.category()) {
            filter.report(LicenseUnneeded.version(pkg, ""));
        } else {
            let deprecated: Vec<_> = licenses.intersection(&self.deprecated).sorted().collect();
            if !deprecated.is_empty() {
                let message = deprecated.iter().join(", ");
                filter.report(LicenseDeprecated.version(pkg, message));
            }
        }
    }
}
