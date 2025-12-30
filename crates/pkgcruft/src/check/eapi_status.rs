use dashmap::DashSet;
use itertools::Itertools;
use pkgcraft::eapi::{EAPI_LATEST_OFFICIAL, EAPIS_OFFICIAL};
use pkgcraft::pkg::{Package, ebuild::EbuildRawPkg};
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::{EapiBanned, EapiDeprecated, EapiUnused};
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    kind: super::CheckKind::EapiStatus,
    reports: &[EapiBanned, EapiDeprecated, EapiUnused],
    scope: Scope::Version,
    sources: &[SourceKind::EbuildRawPkg],
    context: &[],
    create,
}

pub(super) fn create(run: &ScannerRun) -> super::Runner {
    let banned = &run.repo.metadata().config.eapis_banned;
    let unused = if run.enabled(EapiUnused) && !banned.is_empty() {
        EAPIS_OFFICIAL
            .iter()
            .filter(|x| !banned.contains(x.as_str()))
            .filter(|&x| x != &*EAPI_LATEST_OFFICIAL)
            .map(|x| x.to_string())
            .collect()
    } else {
        Default::default()
    };

    Box::new(Check { unused })
}

struct Check {
    unused: DashSet<String>,
}

impl super::CheckRun for Check {
    fn run_ebuild_raw_pkg(&self, pkg: &EbuildRawPkg, run: &ScannerRun) {
        let eapi = pkg.eapi().as_str();
        if run.repo.metadata().config.eapis_deprecated.contains(eapi) {
            EapiDeprecated.version(pkg).message(eapi).report(run);
        } else if run.repo.metadata().config.eapis_banned.contains(eapi) {
            EapiBanned.version(pkg).message(eapi).report(run);
        }

        if run.enabled(EapiUnused) {
            self.unused.remove(eapi);
        }
    }

    fn finish(&self, run: &ScannerRun) {
        if run.enabled(EapiUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            EapiUnused.repo(&run.repo).message(unused).report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pkgcraft::cli::Targets;
    use pkgcraft::config::Config;
    use pkgcraft::eapi::EAPI7;
    use pkgcraft::repo::ebuild::EbuildRepoBuilder;
    use pkgcraft::test::{test_data, test_data_patched};

    use crate::report::Report;
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
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // repo with unused EAPI
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let layout = indoc::indoc! {"
            eapis-banned = 0 1 2 3 4 5 6
        "};
        fs::write(temp.path().join("metadata/layout.conf"), layout).unwrap();
        for eapi in EAPIS_OFFICIAL.iter().filter(|e| **e > &*EAPI7) {
            temp.create_ebuild(format!("cat/pkg-{eapi}"), &[&format!("EAPI={eapi}")])
                .unwrap();
        }
        let mut config = Config::new("pkgcraft", "");
        let repo = Targets::new(&mut config)
            .repo_targets([temp.path()])
            .unwrap()
            .ebuild_repo()
            .unwrap();
        let reports = scanner.run(&repo, &repo).unwrap();
        let expected = vec![
            Report::from_json(
                r#"{"kind":"EapiUnused","target":{"Repo":"test"},"message":"7"}"#,
            )
            .unwrap(),
        ];
        assert_unordered_reports!(reports, expected);

        // secondary with no banned or deprecated EAPIs set
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        assert!(repo.path().join(CHECK).exists());
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
