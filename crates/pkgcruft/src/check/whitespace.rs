use std::collections::HashSet;

use pkgcraft::pkg::ebuild::EbuildRawPkg;

use crate::report::ReportKind::{EapiFormat, WhitespaceInvalid, WhitespaceUnneeded};
use crate::scan::ScannerRun;

use super::EbuildRawPkgCheck;

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

super::register!(Check, super::Check::Whitespace);

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, run: &ScannerRun) {
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
                            .report(run);
                    } else if char_indices.peek().is_none() && !whitespace_only_line {
                        WhitespaceUnneeded
                            .version(pkg)
                            .message("trailing whitespace")
                            .location((lineno, pos + 1))
                            .report(run);
                    }
                }
            }

            // Flag leading single spaces, skipping certain parse tree node variants such
            // as heredocs and raw strings.
            if line.starts_with(' ') {
                let node = pkg
                    .tree()
                    .last_node_for_position(lineno - 1, 0)
                    .unwrap_or_else(|| panic!("nonexistent line: {lineno}"));
                if !self.allowed_leading_whitespace.contains(node.kind()) {
                    WhitespaceUnneeded
                        .version(pkg)
                        .message("leading whitespace")
                        .location(lineno)
                        .report(run);
                }
            } else if whitespace_only_line && !line.is_empty() {
                WhitespaceUnneeded
                    .version(pkg)
                    .location(lineno)
                    .message("empty line with whitespace")
                    .report(run);
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
                        .report(run);
                }
                eapi_assign = true;
            }

            if let Some(prev) = prev_line {
                if prev.trim().is_empty() && whitespace_only_line {
                    WhitespaceUnneeded
                        .version(pkg)
                        .message("empty line")
                        .location(lineno)
                        .report(run);
                }
            }

            prev_line = Some(line);
        }

        if !pkg.data().ends_with('\n') {
            WhitespaceInvalid
                .version(pkg)
                .message("missing ending newline")
                .location(lineno)
                .report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::{test_data, test_data_patched};

    use crate::scan::Scanner;
    use crate::test::{assert_unordered_reports, glob_reports};

    use super::*;

    #[test]
    fn check() {
        let scanner = Scanner::new().reports([CHECK]);

        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
