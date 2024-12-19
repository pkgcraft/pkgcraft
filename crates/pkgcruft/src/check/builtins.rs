use pkgcraft::pkg::{ebuild::EbuildRawPkg, Package};
use std::collections::HashSet;

use crate::report::ReportKind::BuiltinCommand;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildRawPkgCheck};

pub(crate) static CHECK: super::Check = super::Check {
    kind: CheckKind::Builtins,
    scope: Scope::Version,
    source: SourceKind::EbuildRawPkg,
    reports: &[BuiltinCommand],
    context: &[],
};

pub(crate) fn create() -> impl EbuildRawPkgCheck {
    Check {
        commands: ["find", "xargs"]
            .into_iter()
            .map(|name| name.to_string())
            .collect(),
    }
}

struct Check {
    commands: HashSet<String>,
}

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, filter: &mut ReportFilter) {
        let mut cursor = pkg.tree().walk();
        // TODO: use parse tree query
        for node in pkg
            .tree()
            .iter_func()
            .filter(|x| x.kind() == "command_name")
        {
            let cmd_name = node.as_str();
            if self.commands.contains(cmd_name) {
                let cmd = node.parent().unwrap();
                for x in cmd
                    .children(&mut cursor)
                    .iter()
                    .filter(|x| x.kind() == "word")
                {
                    if let Some(builtin) = pkg.eapi().commands().get(x.as_str()) {
                        BuiltinCommand
                            .version(pkg)
                            .message(format!("{cmd_name} uses {builtin}"))
                            .location(&cmd)
                            .report(filter);
                    }
                }
            }
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
