use std::collections::HashSet;

use pkgcraft::pkg::ebuild::EbuildRawPkg;
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::{EapiFormat, WhitespaceInvalid, WhitespaceUnneeded};
use crate::scanner::ReportFilter;
use crate::source::SourceKind;

use super::{CheckKind, EbuildRawPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Whitespace,
    scope: Scope::Version,
    source: SourceKind::EbuildRawPkg,
    reports: &[EapiFormat, WhitespaceInvalid, WhitespaceUnneeded],
    context: &[],
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

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, filter: &mut ReportFilter) {
        let mut prev_line: Option<&str> = None;
        let mut eapi_assign = false;
        let mut lines = pkg.data().lines().peekable();
        let mut lineno = 0;

        while let Some(line) = lines.next() {
            lineno += 1;
            let whitespace_only_line = line.trim().is_empty();

            let mut char_indices = line.char_indices().peekable();
            while let Some((pos, c)) = char_indices.next() {
                // TODO: Check for unnecessary leading whitespace which requires bash
                // parsing to ignore indents inside multiline strings or similar.
                if c.is_whitespace() {
                    if c != ' ' && c != '\t' {
                        WhitespaceInvalid
                            .version(pkg)
                            .message(format!("character {c:?}"))
                            .location((lineno, pos + 1))
                            .report(filter);
                    } else if char_indices.peek().is_none() && !whitespace_only_line {
                        WhitespaceUnneeded
                            .version(pkg)
                            .message("trailing whitespace")
                            .location((lineno, pos + 1))
                            .report(filter);
                    }
                }
            }

            // Flag leading single spaces, skipping certain parse tree node variants such
            // as heredocs and raw strings.
            if line.starts_with(' ') {
                if let Some(node) = pkg.tree().last_node_for_position(lineno - 1, 0) {
                    if !self.allowed_leading_whitespace.contains(node.kind()) {
                        WhitespaceUnneeded
                            .version(pkg)
                            .message("leading whitespace")
                            .location(lineno)
                            .report(filter);
                    }
                }
            } else if whitespace_only_line && !line.is_empty() {
                WhitespaceUnneeded
                    .version(pkg)
                    .location(lineno)
                    .message("empty line with whitespace")
                    .report(filter);
            }

            if !eapi_assign && line.trim().starts_with("EAPI=") {
                if lines.peek().map(|s| !s.is_empty()).unwrap_or_default()
                    || prev_line.map(|s| !s.is_empty()).unwrap_or_default()
                    || !line.starts_with("EAPI=")
                {
                    EapiFormat
                        .version(pkg)
                        .message("non-standard EAPI assignment")
                        .location(lineno)
                        .report(filter);
                }
                eapi_assign = true;
            }

            if let Some(prev) = prev_line {
                if prev.trim().is_empty() && whitespace_only_line {
                    WhitespaceUnneeded
                        .version(pkg)
                        .message("empty line")
                        .location(lineno)
                        .report(filter);
                }
            }

            prev_line = Some(line);
        }

        if !pkg.data().ends_with('\n') {
            WhitespaceInvalid
                .version(pkg)
                .message("missing ending newline")
                .location(lineno)
                .report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let scanner = Scanner::new(repo).checks([CHECK]);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
