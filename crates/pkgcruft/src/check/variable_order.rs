use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildRawPkg;
use strum::{Display, EnumString};

use crate::bash::Tree;
use crate::report::ReportKind::VariableOrder;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildRawPkgCheck};

pub(crate) static CHECK: super::Check = super::Check {
    kind: CheckKind::VariableOrder,
    scope: Scope::Version,
    source: SourceKind::EbuildRawPkg,
    reports: &[VariableOrder],
    context: &[],
    priority: 0,
};

#[derive(Display, EnumString, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
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

pub(crate) fn create() -> impl EbuildRawPkgCheck {
    Check
}

struct Check;

super::register!(Check);

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, tree: &Tree, filter: &mut ReportFilter) {
        let mut variables = vec![];
        for node in tree
            .iter_global_nodes()
            .filter(|node| node.kind() == "variable_assignment")
        {
            // ignore ebuilds with conditionally defined target variables
            if node
                .parent()
                .map(|x| x != tree.root_node())
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
                    .report(filter);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, test_data, test_data_patched};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let data = test_data();
        let repo = data.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, []);
    }
}
