use std::collections::HashMap;

use pkgcraft::bash::Node;
use pkgcraft::dep::Dep;
use pkgcraft::pkg::{ebuild::EbuildRawPkg, Package, RepoPackage};
use pkgcraft::restrict::Scope;
use pkgcraft::traits::Contains;
use tree_sitter::TreeCursor;

use crate::report::ReportKind::{Builtin, Optfeature};
use crate::scanner::ReportFilter;
use crate::source::SourceKind;

use super::{CheckKind, EbuildRawPkgCheck};

pub(crate) static CHECK: super::Check = super::Check {
    kind: CheckKind::Commands,
    scope: Scope::Version,
    source: SourceKind::EbuildRawPkg,
    reports: &[Builtin, Optfeature],
    context: &[],
};

type CommandFn =
    for<'a> fn(&str, &Node<'a>, &mut TreeCursor<'a>, &EbuildRawPkg, &mut ReportFilter);

pub(crate) fn create() -> impl EbuildRawPkgCheck {
    let mut check = Check { commands: Default::default() };
    check.commands.extend(
        ["find", "xargs"]
            .into_iter()
            .map(|name| (name.to_string(), builtins as CommandFn)),
    );
    check.commands.extend(
        ["optfeature"]
            .into_iter()
            .map(|name| (name.to_string(), optfeature as CommandFn)),
    );

    check
}

struct Check {
    commands: HashMap<String, CommandFn>,
}

/// Flag builtins used as external commands.
fn builtins<'a>(
    cmd: &str,
    node: &Node<'a>,
    cursor: &mut TreeCursor<'a>,
    pkg: &EbuildRawPkg,
    filter: &mut ReportFilter,
) {
    for x in node.children(cursor).filter(|x| x.kind() == "word") {
        if let Some(builtin) = pkg.eapi().commands().get(x.as_str()) {
            Builtin
                .version(pkg)
                .message(format!("{cmd} uses {builtin}"))
                .location(node)
                .report(filter);
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
    filter: &mut ReportFilter,
) {
    for x in node.children(cursor).skip(2).filter(|x| x.kind() == "word") {
        match Dep::try_new(x) {
            Ok(dep) => {
                if !pkg.repo().contains(dep.cpn()) {
                    Optfeature
                        .version(pkg)
                        .message(format!("nonexistent dep: {x}"))
                        .location(node)
                        .report(filter);
                }
            }
            Err(_) => {
                Optfeature
                    .version(pkg)
                    .message(format!("invalid dep: {x}"))
                    .location(node)
                    .report(filter);
            }
        }
    }
}

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, filter: &mut ReportFilter) {
        let mut cursor = pkg.tree().walk();
        // TODO: use parse tree query
        for (cmd, node, func) in pkg
            .tree()
            .iter_func()
            .filter(|x| x.kind() == "command_name")
            .filter_map(|x| self.commands.get(x.as_str()).map(|func| (x, func)))
            .filter_map(|(x, func)| x.parent().map(|node| (x.to_string(), node, func)))
        {
            func(&cmd, &node, &mut cursor, pkg, filter);
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
