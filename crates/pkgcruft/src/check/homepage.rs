use std::collections::HashSet;

use pkgcraft::pkg::{ebuild::EbuildPkg, Package};
use pkgcraft::restrict::Scope;
use url::Url;

use crate::iter::ReportFilter;
use crate::report::ReportKind::HomepageInvalid;
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

super::register!(Check);

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
                            .message(format!("unsupported protocol: {url}"))
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

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::scan::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let unneeded = repo.path().join("virtual/HomepageInvalid");
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json", "{unneeded}/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let scanner = Scanner::new(repo).checks([CHECK]);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
