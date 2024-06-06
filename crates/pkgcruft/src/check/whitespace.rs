use std::collections::HashSet;

use pkgcraft::pkg::ebuild::raw::Pkg;

use crate::report::ReportKind::{WhitespaceInvalid, WhitespaceUnneeded};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, RawVersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Whitespace,
    scope: Scope::Version,
    source: SourceKind::EbuildRaw,
    reports: &[WhitespaceInvalid, WhitespaceUnneeded],
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
        let mut prev_line_empty = false;

        for (i, line) in pkg.data().lines().enumerate() {
            for (j, c) in line.chars().enumerate() {
                // TODO: Check for unnecessary leading whitespace which requires bash
                // parsing to ignore indents inside multiline strings or similar.
                if j == line.len() - 1 && (c == ' ' || c == '\t') {
                    let message = "trailing whitespace";
                    filter.report(WhitespaceUnneeded.version(pkg, message).line(i + 1));
                } else if c.is_whitespace() && !self.allowed.contains(&c) {
                    let message = format!("position {:04}: {c:?}", j + 1);
                    filter.report(WhitespaceInvalid.version(pkg, message).line(i + 1));
                }
            }

            if !line.trim().is_empty() {
                prev_line_empty = false;
            } else if prev_line_empty {
                filter.report(WhitespaceUnneeded.version(pkg, "empty line").line(i + 1));
            } else {
                prev_line_empty = true;
            }
        }
    }
}
