use std::collections::HashSet;

use pkgcraft::pkg::ebuild::raw::Pkg;

use crate::bash::Tree;
use crate::report::ReportKind::{EapiFormat, WhitespaceInvalid, WhitespaceUnneeded};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildRawPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Whitespace,
    scope: Scope::Version,
    source: SourceKind::EbuildRawPkg,
    reports: &[EapiFormat, WhitespaceInvalid, WhitespaceUnneeded],
    context: &[],
    priority: 0,
};

pub(super) fn create() -> impl EbuildRawPkgCheck {
    Check {
        allowed_leading_whitespace: ["heredoc_body", "raw_string"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    }
}

struct Check {
    allowed_leading_whitespace: HashSet<String>,
}

super::register!(Check);

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &Pkg, tree: &Tree, filter: &mut ReportFilter) {
        let mut prev_line: Option<&str> = None;
        let mut eapi_assign = false;
        let mut lines = pkg.data().lines().peekable();
        let mut lineno = 0;

        while let Some(line) = lines.next() {
            lineno += 1;
            let mut char_indices = line.char_indices().peekable();
            while let Some((pos, c)) = char_indices.next() {
                // TODO: Check for unnecessary leading whitespace which requires bash
                // parsing to ignore indents inside multiline strings or similar.
                if c.is_whitespace() {
                    if c != ' ' && c != '\t' {
                        let message = format!("character {c:?}");
                        let report = WhitespaceInvalid.version(pkg, message);
                        filter.report(report.location((lineno, pos + 1)));
                    } else if char_indices.peek().is_none() {
                        let message = "trailing whitespace";
                        let report = WhitespaceUnneeded.version(pkg, message);
                        filter.report(report.location((lineno, pos + 1)));
                    }
                }
            }

            // Flag leading single spaces, skipping certain parse tree node variants such
            // as heredocs and raw strings.
            if line.starts_with(' ') {
                if let Some(node) = tree.last_node_for_position(lineno - 1, 0) {
                    if !self.allowed_leading_whitespace.contains(node.kind()) {
                        let message = "leading whitespace";
                        let report = WhitespaceUnneeded.version(pkg, message);
                        filter.report(report.location(lineno));
                    }
                }
            }

            if !eapi_assign && line.trim().starts_with("EAPI=") {
                if lines.peek().map(|s| !s.is_empty()).unwrap_or_default()
                    || prev_line.map(|s| !s.is_empty()).unwrap_or_default()
                    || !line.starts_with("EAPI=")
                {
                    let message = "non-standard EAPI assignment";
                    filter.report(EapiFormat.version(pkg, message).location(lineno));
                }
                eapi_assign = true;
            }

            if let Some(prev) = prev_line {
                if prev.trim().is_empty() && line.trim().is_empty() {
                    let report = WhitespaceUnneeded.version(pkg, "empty line");
                    filter.report(report.location(lineno));
                }
            }

            prev_line = Some(line);
        }

        if !pkg.data().ends_with('\n') {
            let message = "missing ending newline";
            let report = WhitespaceInvalid.version(pkg, message);
            filter.report(report.location(lineno));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{TEST_DATA, TEST_DATA_PATCHED};
    use pretty_assertions::assert_eq;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
