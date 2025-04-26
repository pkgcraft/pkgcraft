use std::collections::HashSet;

use pkgcraft::pkg::{Package, ebuild::EbuildPkg};
use url::Url;

use crate::report::ReportKind::HomepageInvalid;
use crate::scan::ScannerRun;

use super::EbuildPkgCheck;

pub(super) fn create() -> impl EbuildPkgCheck {
    Check {
        allowed_protocols: ["http", "https"].into_iter().map(Into::into).collect(),
        missing_categories: ["acct-group", "acct-user", "virtual"]
            .iter()
            .map(|x| x.to_string())
            .collect(),
    }
}

static CHECK: super::Check = super::Check::Homepage;

struct Check {
    allowed_protocols: HashSet<String>,
    missing_categories: HashSet<String>,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, run: &ScannerRun) {
        let allowed_missing = self.missing_categories.contains(pkg.category());
        if pkg.homepage().is_empty() {
            if !allowed_missing {
                HomepageInvalid.version(pkg).message("missing").report(run);
            }
        } else if allowed_missing {
            HomepageInvalid.version(pkg).message("unneeded").report(run);
        }

        for value in pkg.homepage() {
            match Url::parse(value) {
                Ok(url) => {
                    let protocol = url.scheme();
                    if !self.allowed_protocols.contains(protocol) {
                        HomepageInvalid
                            .version(pkg)
                            .message(format!("unsupported protocol: {url}"))
                            .report(run);
                    }
                }
                Err(e) => {
                    HomepageInvalid
                        .version(pkg)
                        .message(format!("{e}: {value}"))
                        .report(run);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::{test_data, test_data_patched};

    use crate::scan::Scanner;
    use crate::test::{assert_unordered_reports, glob_reports};

    use super::*;

    #[test]
    fn check() {
        let scanner = Scanner::new().reports([CHECK]);

        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let unneeded = repo.path().join("virtual/HomepageInvalid");
        let expected = glob_reports!("{dir}/*/reports.json", "{unneeded}/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
