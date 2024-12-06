use pkgcraft::pkg::{ebuild::EbuildRawPkg, Package};
use std::collections::HashMap;

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
    priority: 0,
};

type TestFn = fn(&str, &EbuildRawPkg) -> Option<String>;

// TODO: scan for multiple -exec uses and -execdir?
fn find(cmd: &str, pkg: &EbuildRawPkg) -> Option<String> {
    cmd.split_whitespace()
        .skip_while(|x| *x != "-exec")
        .nth(1)
        .and_then(|x| pkg.eapi().commands().get(x))
        .map(|x| x.to_string())
}

fn xargs(cmd: &str, pkg: &EbuildRawPkg) -> Option<String> {
    cmd.split_whitespace()
        .nth(1)
        .and_then(|x| pkg.eapi().commands().get(x))
        .map(|x| x.to_string())
}

pub(crate) fn create() -> impl EbuildRawPkgCheck {
    Check {
        commands: [("find", find as TestFn), ("xargs", xargs as TestFn)]
            .into_iter()
            .map(|(name, x)| (name.to_string(), x))
            .collect(),
    }
}

struct Check {
    commands: HashMap<String, TestFn>,
}

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, filter: &mut ReportFilter) {
        // TODO: use parse tree query
        for node in pkg
            .tree()
            .iter_func()
            .filter(|x| x.kind() == "command_name")
        {
            let cmd_name = node.as_str();
            if let Some(func) = self.commands.get(cmd_name) {
                let cmd = node.parent().unwrap();
                if let Some(builtin) = func(cmd.as_str(), pkg) {
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
