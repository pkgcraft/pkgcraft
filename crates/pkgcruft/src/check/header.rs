use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildRawPkg;
use regex::Regex;

use crate::report::ReportKind::HeaderInvalid;
use crate::scan::ScannerRun;

use super::EbuildRawPkgCheck;

static GENTOO_LICENSE_HEADER: &str =
    "# Distributed under the terms of the GNU General Public License v2";

pub(super) fn create() -> impl EbuildRawPkgCheck {
    Check {
        copyright_re: Regex::new(
            r"^# Copyright ((?P<begin>\d{4})-)?(?P<end>\d{4}) (?P<holder>.+)$",
        )
        .unwrap(),
    }
}

struct Check {
    copyright_re: Regex,
}

super::register!(Check, super::Check::Header);

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, run: &ScannerRun) {
        let lines: Vec<_> = pkg
            .data()
            .lines()
            .take(2)
            .filter(|x| x.starts_with('#'))
            .collect();
        let Some((copyright, license)) = lines.into_iter().collect_tuple() else {
            HeaderInvalid
                .version(pkg)
                .message("missing copyright and/or license")
                .report(run);
            return;
        };

        if let Some(m) = self.copyright_re.captures(copyright.trim()) {
            // Copyright policy is active since 2018-10-21 via GLEP 76, so it applies to all
            // ebuilds committed in 2019 and later.
            let end: u64 = m.name("end").unwrap().as_str().parse().unwrap();
            if end >= 2019 {
                let holder = m.name("holder").unwrap().as_str();
                if holder != "Gentoo Authors" {
                    HeaderInvalid
                        .version(pkg)
                        .message(format!("invalid copyright holder: {holder}"))
                        .location(1)
                        .report(run);
                }
            }
        } else {
            HeaderInvalid
                .version(pkg)
                .message(format!("invalid copyright: {copyright}"))
                .location(1)
                .report(run);
        }

        if license != GENTOO_LICENSE_HEADER {
            HeaderInvalid
                .version(pkg)
                .message(format!("invalid license: {license}"))
                .location(2)
                .report(run);
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
        // check isn't run by default in non-gentoo repo
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let scanner = Scanner::new();
        let mut reports = scanner.run(repo, repo).unwrap();
        assert!(!reports.any(|r| CHECK.reports().contains(&r.kind)));

        let scanner = Scanner::new().reports([CHECK]);

        // check explicitly run in non-gentoo repo
        let mut reports = scanner.run(repo, repo).unwrap();
        assert!(reports.any(|r| CHECK.reports().contains(&r.kind)));

        // gentoo unfixed
        let repo = data.ebuild_repo("gentoo").unwrap();
        let dir = repo.path().join(CHECK);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // gentoo fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("gentoo").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
