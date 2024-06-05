use once_cell::sync::Lazy;
use pkgcraft::pkg::ebuild::raw::Pkg;
use regex::Regex;

use crate::report::ReportKind::{
    HeaderCopyrightInvalid, HeaderCopyrightOutdated, HeaderLicenseInvalid,
};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, RawVersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Header,
    scope: Scope::Version,
    source: SourceKind::EbuildRaw,
    reports: &[HeaderCopyrightInvalid, HeaderCopyrightOutdated, HeaderLicenseInvalid],
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
        if let Some(m) = COPYRIGHT_REGEX.captures(line) {
            // Copyright policy is active since 2018-10-21 via GLEP 76, so it applies to all
            // ebuilds committed in 2019 and later.
            let end: u64 = m.name("end").unwrap().as_str().parse().unwrap();
            if end >= 2019 {
                let holder = m.name("holder").unwrap().as_str();
                if holder == "Gentoo Foundation" {
                    filter.report(HeaderCopyrightOutdated.version(pkg, format!("{line:?}")));
                } else if holder != "Gentoo Authors" {
                    filter.report(HeaderCopyrightInvalid.version(pkg, format!("{line:?}")));
                }
            }
        } else {
            filter.report(HeaderCopyrightInvalid.version(pkg, format!("{line:?}")));
        }

        line = lines.next().unwrap_or_default();
        if line != GENTOO_LICENSE_HEADER {
            let message = if line.trim().is_empty() {
                "missing license header".to_string()
            } else {
                format!("{line:?}")
            };
            filter.report(HeaderLicenseInvalid.version(pkg, message));
        }
    }
}
