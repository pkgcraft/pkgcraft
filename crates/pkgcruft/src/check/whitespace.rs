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
    Check
}

struct Check;

super::register!(Check);

impl RawVersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        let mut prev_line_empty = false;
        let mut lines = pkg.data().lines().peekable();
        let mut lineno = 0;

        while let Some(line) = lines.next() {
            lineno += 1;
            let mut chars = line.chars().peekable();
            let mut pos = 0;
            while let Some(c) = chars.next() {
                pos += 1;
                // TODO: Check for unnecessary leading whitespace which requires bash
                // parsing to ignore indents inside multiline strings or similar.
                if c.is_whitespace() {
                    if c != ' ' && c != '\t' {
                        let message = format!("character {pos:04}: {c:?}");
                        filter.report(WhitespaceInvalid.version(pkg, message).line(lineno));
                    } else if chars.peek().is_none() {
                        let message = "trailing whitespace";
                        filter.report(WhitespaceUnneeded.version(pkg, message).line(lineno));
                    }
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

        if !pkg.data().ends_with('\n') {
            let message = "missing ending newline";
            filter.report(WhitespaceInvalid.version(pkg, message).line(lineno));
        }
    }
}
