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
        let mut lines = pkg.data().lines().peekable();
        let mut lineno = 0;

        while let Some(line) = lines.next() {
            lineno += 1;
            for (i, c) in line.chars().enumerate() {
                // TODO: Check for unnecessary leading whitespace which requires bash
                // parsing to ignore indents inside multiline strings or similar.
                if i == line.len() - 1 && (c == ' ' || c == '\t') {
                    let message = "trailing whitespace";
                    filter.report(WhitespaceUnneeded.version(pkg, message).line(lineno));
                } else if c.is_whitespace() && !self.allowed.contains(&c) {
                    let message = format!("position {:04}: {c:?}", lineno);
                    filter.report(WhitespaceInvalid.version(pkg, message).line(lineno));
                }
            }

            if !line.trim().is_empty() {
                prev_line_empty = false;
            } else if prev_line_empty || lines.peek().is_none() {
                filter.report(WhitespaceUnneeded.version(pkg, "empty line").line(lineno));
            } else {
                prev_line_empty = true;
            }
        }
    }
}
