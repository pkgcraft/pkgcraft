use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildRawPkg;
use pkgcraft::restrict::Scope;
use strum::{Display, EnumString};

use crate::report::ReportKind::VariableOrder;
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    super::Check {
        kind: super::CheckKind::VariableOrder,
        reports: &[VariableOrder],
        scope: Scope::Version,
        sources: &[SourceKind::EbuildRawPkg],
        context: &[],
        create,
    }
}

#[derive(Display, EnumString, PartialEq, Eq, PartialOrd, Ord)]
#[strum(serialize_all = "UPPERCASE")]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
enum Variable {
    DESCRIPTION,
    HOMEPAGE,
    SRC_URI,
    S,
    LICENSE,
    SLOT,
    KEYWORDS,
    IUSE,
    RESTRICT,
    PROPERTIES,
}

pub(super) fn create(_run: &ScannerRun) -> super::Runner {
    Box::new(Check)
}

struct Check;

impl super::CheckRun for Check {
    fn run_ebuild_raw_pkg(&self, pkg: &EbuildRawPkg, run: &ScannerRun) {
        let mut variables = vec![];
        for node in pkg
            .tree()
            .iter_global()
            .filter(|node| node.kind() == "variable_assignment")
        {
            // ignore ebuilds with conditionally defined target variables
            if node
                .parent()
                .map(|x| x != pkg.tree().root_node())
                .unwrap_or_default()
            {
                return;
            }

            let name = node.name().expect("unnamed variable");
            if let Ok(var) = name.parse::<Variable>() {
                variables.push((var, node.line()));
            }
        }

        for ((var1, _), (var2, lineno)) in variables.iter().tuple_windows() {
            if var2 < var1 {
                VariableOrder
                    .version(pkg)
                    .message(format!("{var2} should occur before {var1}"))
                    .location(*lineno)
                    .report(run);
            }
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
