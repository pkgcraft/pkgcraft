use std::collections::HashSet;

use pkgcraft::pkg::{ebuild::EbuildPkg, Package};
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
        missing_categories: ["acct-group", "acct-user", "virtual"]
            .iter()
            .map(|x| x.to_string())
            .collect(),
    }
}

struct Check {
    allowed_protocols: HashSet<String>,
    missing_categories: HashSet<String>,
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        let allowed_missing = self.missing_categories.contains(pkg.category());
        if pkg.homepage().is_empty() {
            if !allowed_missing {
                HomepageInvalid
                    .version(pkg)
                    .message("missing")
                    .report(filter);
            }
        } else if allowed_missing {
            HomepageInvalid
                .version(pkg)
                .message("unneeded")
                .report(filter);
        }

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
