use std::sync::LazyLock;

use pkgcraft::pkg::ebuild::EbuildRawPkg;
use regex::Regex;

use crate::bash::Tree;
use crate::report::ReportKind::HeaderInvalid;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, EbuildRawPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Header,
    scope: Scope::Version,
    source: SourceKind::EbuildRawPkg,
    reports: &[HeaderInvalid],
    context: &[CheckContext::Gentoo],
    priority: 0,
};

static COPYRIGHT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^# Copyright ((?P<begin>\d{4})-)?(?P<end>\d{4}) (?P<holder>.+)$").unwrap()
});

static GENTOO_LICENSE_HEADER: &str =
    "# Distributed under the terms of the GNU General Public License v2";

pub(super) fn create() -> impl EbuildRawPkgCheck {
    Check
}

struct Check;

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, _tree: &Tree, filter: &mut ReportFilter) {
        let mut lines = pkg.data().lines();

        let mut line = lines.next().unwrap_or_default();
        if let Some(m) = COPYRIGHT_REGEX.captures(line.trim()) {
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
            let mut report = HeaderInvalid.version(pkg);

            if !line.trim().starts_with('#') || line.trim().is_empty() {
                report = report.message("missing copyright header");
            } else {
                report = report
                    .message(format!("invalid copyright: {line}"))
                    .location(1);
            };

            report.report(filter);
        }

        line = lines.next().unwrap_or_default();
        if line != GENTOO_LICENSE_HEADER {
            let mut report = HeaderInvalid.version(pkg);

            if !line.trim().starts_with('#') || line.trim().is_empty() {
                report = report.message("missing license header");
            } else {
                report = report
                    .message(format!("invalid license: {line}"))
                    .location(2);
            };

            report.report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, test_data, test_data_patched};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // gentoo unfixed
        let data = test_data();
        let repo = data.repo("gentoo").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, expected);

        // gentoo fixed
        let data = test_data_patched();
        let repo = data.repo("gentoo").unwrap();
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, []);
    }
}
