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

        let copyright = lines.next().unwrap_or_default();
        if let Some(m) = COPYRIGHT_REGEX.captures(copyright) {
            // Copyright policy is active since 2018-10-21 via GLEP 76, so it applies to all
            // ebuilds committed in 2019 and later.
            let end: u64 = m.name("end").unwrap().as_str().parse().unwrap();
            if end >= 2019 {
                let holder = m.name("holder").unwrap().as_str();
                if holder == "Gentoo Foundation" {
                    let message = format!("{copyright:?}");
                    filter.report(HeaderCopyrightOutdated.version(pkg, message));
                } else if holder != "Gentoo Authors" {
                    let message = format!("{copyright:?}");
                    filter.report(HeaderCopyrightInvalid.version(pkg, message));
                }
            }
        } else {
            let message = format!("{copyright:?}");
            filter.report(HeaderCopyrightInvalid.version(pkg, message));
        }

        let license = lines.next().unwrap_or_default();
        if license != GENTOO_LICENSE_HEADER {
            let message = if license.trim().is_empty() {
                "missing license header".to_string()
            } else {
                format!("{license:?}")
            };
            filter.report(HeaderLicenseInvalid.version(pkg, message));
        }
    }
}
