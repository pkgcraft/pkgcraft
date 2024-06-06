use std::collections::HashSet;

use pkgcraft::pkg::ebuild::raw::Pkg;

use crate::report::ReportKind::WhitespaceInvalid;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, RawVersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Whitespace,
    scope: Scope::Version,
    source: SourceKind::EbuildRaw,
    reports: &[WhitespaceInvalid],
    context: &[],
    priority: 0,
};

pub(super) fn create() -> impl RawVersionCheck {
    Check {
        allowed: [' ', '\t', '\n'].into_iter().collect(),
    }
}

struct Check {
    allowed: HashSet<char>,
}

super::register!(Check);

impl RawVersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        for (i, line) in pkg.data().lines().enumerate() {
            if line
                .chars()
                .any(|c| c.is_whitespace() && !self.allowed.contains(&c))
            {
                let message = format!("{line:?}");
                filter.report(WhitespaceInvalid.version(pkg, message).line(i + 1));
            }
        }
    }
}
