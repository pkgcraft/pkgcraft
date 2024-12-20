use pkgcraft::pkg::ebuild::EbuildPkg;
use url::Url;

use crate::report::ReportKind::HomepageInvalid;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Homepage,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[HomepageInvalid],
    context: &[],
};

pub(super) fn create() -> impl EbuildPkgCheck {
    Check
}

struct Check;

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        for value in pkg.homepage() {
            if let Err(e) = Url::parse(value) {
                HomepageInvalid
                    .version(pkg)
                    .message(format!("{e}: {value}"))
                    .report(filter);
            }
        }
    }
}
