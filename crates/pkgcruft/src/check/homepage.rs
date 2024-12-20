use std::collections::HashSet;

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
    Check {
        allowed_protocols: ["http", "https"].into_iter().map(Into::into).collect(),
    }
}

struct Check {
    allowed_protocols: HashSet<String>,
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        for value in pkg.homepage() {
            match Url::parse(value) {
                Ok(url) => {
                    let protocol = url.scheme();
                    if !self.allowed_protocols.contains(protocol) {
                        HomepageInvalid
                            .version(pkg)
                            .message(format!("unsupported protocol: {protocol}"))
                            .report(filter);
                    }
                }
                Err(e) => {
                    HomepageInvalid
                        .version(pkg)
                        .message(format!("{e}: {value}"))
                        .report(filter);
                }
            }
        }
    }
}
