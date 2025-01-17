use std::collections::HashMap;

use pkgcraft::bash::Node;
use pkgcraft::dep::Dep;
use pkgcraft::pkg::{ebuild::EbuildRawPkg, Package, RepoPackage};
use pkgcraft::traits::Contains;
use tree_sitter::TreeCursor;

use crate::report::ReportKind::{Builtin, Optfeature};
use crate::scan::ScannerRun;

use super::EbuildRawPkgCheck;

type CommandFn = for<'a> fn(&str, &Node<'a>, &mut TreeCursor<'a>, &EbuildRawPkg, &ScannerRun);

pub(crate) fn create() -> impl EbuildRawPkgCheck {
    let mut check = Check { commands: Default::default() };
    check.extend(["find", "xargs"], builtins);
    check.extend(["optfeature"], optfeature);
    check
}

static CHECK: super::Check = super::Check::Commands;

struct Check {
    commands: HashMap<String, CommandFn>,
}

impl Check {
    /// Register commands for the check to handle.
    fn extend<I>(&mut self, names: I, func: CommandFn)
    where
        I: IntoIterator,
        I::Item: std::fmt::Display,
    {
        self.commands
            .extend(names.into_iter().map(|x| (x.to_string(), func)));
    }
}

super::register!(Check);

/// Flag builtins used as external commands.
fn builtins<'a>(
    cmd: &str,
    node: &Node<'a>,
    cursor: &mut TreeCursor<'a>,
    pkg: &EbuildRawPkg,
    run: &ScannerRun,
) {
    for x in node.children(cursor).filter(|x| x.kind() == "word") {
        if let Some(builtin) = pkg.eapi().commands().get(x.as_str()) {
            Builtin
                .version(pkg)
                .message(format!("{cmd} uses {builtin}"))
                .location(node)
                .report(run);
        }
    }
}

// TODO: handle multi-dep arguments and USE flag queries
/// Flag issues with optfeature usage.
fn optfeature<'a>(
    _cmd: &str,
    node: &Node<'a>,
    cursor: &mut TreeCursor<'a>,
    pkg: &EbuildRawPkg,
    run: &ScannerRun,
) {
    for x in node.children(cursor).skip(2).filter(|x| x.kind() == "word") {
        match Dep::try_new(x) {
            Ok(dep) => {
                // TODO: move inherited repo search to pkgcraft
                if !pkg.repo().trees().rev().any(|r| r.contains(dep.cpn())) {
                    Optfeature
                        .version(pkg)
                        .message(format!("nonexistent dep: {x}"))
                        .location(node)
                        .report(run);
                }
            }
            Err(_) => {
                Optfeature
                    .version(pkg)
                    .message(format!("invalid dep: {x}"))
                    .location(node)
                    .report(run);
            }
        }
    }
}

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, run: &ScannerRun) {
        let mut cursor = pkg.tree().walk();
        // TODO: use parse tree query
        for (cmd, node, func) in pkg
            .tree()
            .iter_func()
            .filter(|x| x.kind() == "command_name")
            .filter_map(|x| self.commands.get(x.as_str()).map(|func| (x, func)))
            .filter_map(|(x, func)| x.parent().map(|node| (x.to_string(), node, func)))
        {
            func(&cmd, &node, &mut cursor, pkg, run);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::scan::Scanner;
    use crate::test::glob_reports;

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
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
