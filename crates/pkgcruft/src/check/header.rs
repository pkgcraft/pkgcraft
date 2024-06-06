use once_cell::sync::Lazy;
use pkgcraft::pkg::ebuild::raw::Pkg;
use regex::Regex;

use crate::report::ReportKind::HeaderInvalid;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, RawVersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Header,
    scope: Scope::Version,
    source: SourceKind::EbuildRaw,
    reports: &[HeaderInvalid],
    context: &[CheckContext::Gentoo],
    priority: 0,
};

static COPYRIGHT_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^# Copyright ((?P<begin>\d{4})-)?(?P<end>\d{4}) (?P<holder>.+)$").unwrap()
});

static GENTOO_LICENSE_HEADER: &str =
    "# Distributed under the terms of the GNU General Public License v2";

pub(super) fn create() -> impl RawVersionCheck {
    Check
}

struct Check;

super::register!(Check);

impl RawVersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        let mut lines = pkg.data().lines();

        let mut line = lines.next().unwrap_or_default();
        if let Some(m) = COPYRIGHT_REGEX.captures(line.trim()) {
            // Copyright policy is active since 2018-10-21 via GLEP 76, so it applies to all
            // ebuilds committed in 2019 and later.
            let end: u64 = m.name("end").unwrap().as_str().parse().unwrap();
            if end >= 2019 {
                let holder = m.name("holder").unwrap().as_str();
                if holder != "Gentoo Authors" {
                    let message = format!("invalid copyright holder: {holder}");
                    filter.report(HeaderInvalid.version(pkg, message));
                }
            }
        } else {
            let message = if !line.trim().starts_with('#') || line.trim().is_empty() {
                "missing copyright header".to_string()
            } else {
                format!("invalid copyright: {line}")
            };

            filter.report(HeaderInvalid.version(pkg, message));
        }

        line = lines.next().unwrap_or_default();
        if line != GENTOO_LICENSE_HEADER {
            let message = if !line.trim().starts_with('#') || line.trim().is_empty() {
                "missing license header".to_string()
            } else {
                format!("invalid license: {line}")
            };

            filter.report(HeaderInvalid.version(pkg, message));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;
    use pretty_assertions::assert_eq;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // gentoo unfixed
        let repo = TEST_DATA.repo("gentoo").unwrap();
        let check_dir = repo.path().join(CHECK);
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{check_dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);
    }
}
