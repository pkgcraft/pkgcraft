use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildRawPkg;
use pkgcraft::restrict::Scope;
use regex::Regex;

use crate::iter::ReportFilter;
use crate::report::ReportKind::HeaderInvalid;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, EbuildRawPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Header,
    scope: Scope::Version,
    source: SourceKind::EbuildRawPkg,
    reports: &[HeaderInvalid],
    context: &[CheckContext::Gentoo],
};

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

super::register!(Check);

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, filter: &ReportFilter) {
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
                .report(filter);
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
                        .report(filter);
                }
            }
        } else {
            HeaderInvalid
                .version(pkg)
                .message(format!("invalid copyright: {copyright}"))
                .location(1)
                .report(filter);
        }

        if license != GENTOO_LICENSE_HEADER {
            HeaderInvalid
                .version(pkg)
                .message(format!("invalid license: {license}"))
                .location(2)
                .report(filter);
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
        // gentoo unfixed
        let data = test_data();
        let repo = data.ebuild_repo("gentoo").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // gentoo fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("gentoo").unwrap();
        let scanner = Scanner::new(repo).checks([CHECK]);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
