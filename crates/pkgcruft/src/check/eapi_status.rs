use dashmap::DashSet;
use itertools::Itertools;
use pkgcraft::eapi::{EAPIS_OFFICIAL, EAPI_LATEST_OFFICIAL};
use pkgcraft::pkg::{ebuild::EbuildRawPkg, Package};
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::iter::ReportFilter;
use crate::report::ReportKind::{EapiBanned, EapiDeprecated, EapiUnused};

use super::EbuildRawPkgCheck;

pub(super) fn create(repo: &EbuildRepo, filter: &ReportFilter) -> impl EbuildRawPkgCheck {
    let banned = &repo.metadata().config.eapis_banned;
    let unused = if filter.enabled(EapiUnused) && !banned.is_empty() {
        EAPIS_OFFICIAL
            .iter()
            .filter(|x| !banned.contains(x.as_str()))
            .filter(|&x| x != &*EAPI_LATEST_OFFICIAL)
            .map(|x| x.to_string())
            .collect()
    } else {
        Default::default()
    };

    Check { repo: repo.clone(), unused }
}

static CHECK: super::Check = super::Check::EapiStatus;

struct Check {
    repo: EbuildRepo,
    unused: DashSet<String>,
}

super::register!(Check);

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, filter: &ReportFilter) {
        let eapi = pkg.eapi().as_str();
        if self.repo.metadata().config.eapis_deprecated.contains(eapi) {
            EapiDeprecated.version(pkg).message(eapi).report(filter);
        } else if self.repo.metadata().config.eapis_banned.contains(eapi) {
            EapiBanned.version(pkg).message(eapi).report(filter);
        }

        if filter.enabled(EapiUnused) {
            self.unused.remove(eapi);
        }
    }

    fn finish_check(&self, repo: &EbuildRepo, filter: &ReportFilter) {
        if filter.enabled(EapiUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            EapiUnused.repo(repo).message(unused).report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pkgcraft::config::Config;
    use pkgcraft::repo::ebuild::EbuildRepoBuilder;
    use pkgcraft::test::*;

    use crate::report::Report;
    use crate::scan::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        let scanner = Scanner::new().reports([CHECK]);

        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // TODO: move this to shared test data
        // repo with unused EAPI
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let layout = indoc::indoc! {"
            eapis-banned = 0 1 2 3 4 5 6
        "};
        fs::write(temp.path().join("metadata/layout.conf"), layout).unwrap();
        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        let mut config = Config::new("pkgcraft", "");
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();
        let reports = scanner.run(&repo, &repo).unwrap();
        let expected = vec![Report::from_json(
            r#"{"kind":"EapiUnused","scope":{"Repo":"test"},"message":"7"}"#,
        )
        .unwrap()];
        assert_unordered_eq!(reports, expected);

        // secondary with no banned or deprecated EAPIs set
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        assert!(repo.path().join(CHECK).exists());
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
