use std::collections::HashMap;

use pkgcraft::bash::Node;
use pkgcraft::dep::Dep;
use pkgcraft::eapi::{EAPIS, Eapi};
use pkgcraft::pkg::{Package, RepoPackage, ebuild::EbuildRawPkg};
use pkgcraft::restrict::Scope;
use pkgcraft::shell::phase::PhaseKind;
use pkgcraft::traits::Contains;
use tree_sitter::TreeCursor;

use crate::report::ReportKind::{
    Builtin, CommandDieUnneeded, CommandScopeInvalid, Optfeature, PhaseCall,
};
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    kind: super::CheckKind::Commands,
    reports: &[
        Builtin,
        CommandDieUnneeded,
        CommandScopeInvalid,
        Optfeature,
        PhaseCall,
    ],
    scope: Scope::Version,
    sources: &[SourceKind::EbuildRawPkg],
    context: &[],
    create,
}

type CommandFn =
    for<'a> fn(&str, &Node<'a>, &Node<'a>, &mut TreeCursor<'a>, &EbuildRawPkg, &ScannerRun);

pub(super) fn create(_run: &ScannerRun) -> super::Runner {
    let mut check = Check { commands: Default::default() };

    // register non-EAPI commands
    check.register(["find", "xargs"], builtins);
    check.register(["optfeature"], optfeature);

    // register EAPI commands
    for eapi in &*EAPIS {
        check.register_eapi(eapi, eapi.commands(), eapi_command);
    }

    Box::new(check)
}

struct Check {
    commands: HashMap<&'static Eapi, HashMap<String, Vec<CommandFn>>>,
}

impl Check {
    /// Register commands for the check to handle.
    fn register<I>(&mut self, names: I, func: CommandFn)
    where
        I: IntoIterator,
        I::Item: std::fmt::Display,
    {
        for name in names {
            for eapi in &*EAPIS {
                self.commands
                    .entry(eapi)
                    .or_default()
                    .entry(name.to_string())
                    .or_default()
                    .push(func);
            }
        }
    }

    /// Register EAPI commands for the check to handle.
    fn register_eapi<I>(&mut self, eapi: &'static Eapi, names: I, func: CommandFn)
    where
        I: IntoIterator,
        I::Item: std::fmt::Display,
    {
        for name in names {
            self.commands
                .entry(eapi)
                .or_default()
                .entry(name.to_string())
                .or_default()
                .push(func);
        }
    }
}

/// Flag builtins used as external commands.
fn builtins<'a>(
    cmd: &str,
    _func_node: &Node<'a>,
    cmd_node: &Node<'a>,
    cursor: &mut TreeCursor<'a>,
    pkg: &EbuildRawPkg,
    run: &ScannerRun,
) {
    for x in cmd_node.children(cursor).filter(|x| x.kind() == "word") {
        if let Some(builtin) = pkg.eapi().commands().get(x.as_str()) {
            Builtin
                .version(pkg)
                .message(format!("{cmd} uses {builtin}"))
                .location(cmd_node)
                .report(run);
        }
    }
}

// TODO: handle multi-dep arguments and USE flag queries
/// Flag issues with optfeature usage.
fn optfeature<'a>(
    _cmd: &str,
    _func_node: &Node<'a>,
    cmd_node: &Node<'a>,
    cursor: &mut TreeCursor<'a>,
    pkg: &EbuildRawPkg,
    run: &ScannerRun,
) {
    for x in cmd_node
        .children(cursor)
        .skip(2)
        .filter(|x| x.kind() == "word")
    {
        match Dep::try_new(x) {
            Ok(dep) => {
                // TODO: move inherited repo search to pkgcraft
                if !pkg.repo().trees().any(|r| r.contains(dep.cpn())) {
                    Optfeature
                        .version(pkg)
                        .message(format!("nonexistent dep: {x}"))
                        .location(cmd_node)
                        .report(run);
                }
            }
            Err(_) => {
                Optfeature
                    .version(pkg)
                    .message(format!("invalid dep: {x}"))
                    .location(cmd_node)
                    .report(run);
            }
        }
    }
}

/// Flag issues with EAPI commands.
fn eapi_command<'a>(
    cmd: &str,
    func_node: &Node<'a>,
    cmd_node: &Node<'a>,
    _cursor: &mut TreeCursor<'a>,
    pkg: &EbuildRawPkg,
    run: &ScannerRun,
) {
    let eapi_cmd = pkg.eapi().commands().get(cmd).unwrap();
    let func_name = func_node.name().unwrap_or_default();
    // TODO: handle nested function calls
    if let Ok(phase) = cmd.parse::<PhaseKind>() {
        PhaseCall
            .version(pkg)
            .message(format!("{phase}"))
            .location(cmd_node)
            .report(run);
    } else if let Ok(phase) = func_name.parse::<PhaseKind>()
        && !eapi_cmd.is_allowed(&phase)
    {
        CommandScopeInvalid
            .version(pkg)
            .message(format!("{cmd}: disabled in {phase} scope"))
            .location(cmd_node)
            .report(run);
    }

    // flag unnecessary `die` usage for fatal EAPI commands
    if eapi_cmd.die_on_failure {
        // Note that this only flags simple commands and not command lists, for example
        // `emake || die` is flagged, but not `true && emake || die`.
        if let Some(node) = cmd_node
            .next_sibling()
            .filter(|x| x.kind() == "||")
            .and_then(|x| x.next_sibling())
            && node
                .into_iter()
                .next()
                .is_some_and(|x| x.kind() == "command_name" && x.as_str() == "die")
        {
            CommandDieUnneeded
                .version(pkg)
                .message(cmd)
                .location(cmd_node)
                .report(run);
        }
    }
}

impl super::CheckRun for Check {
    fn run_ebuild_raw_pkg(&self, pkg: &EbuildRawPkg, run: &ScannerRun) {
        let eapi = pkg.eapi();
        let cmds = self
            .commands
            .get(eapi)
            .unwrap_or_else(|| panic!("{pkg}: no commands registered for EAPI {eapi}"));

        let mut cursor = pkg.tree().walk();
        for func_node in pkg.tree().iter_func() {
            for (cmd, cmd_node, funcs) in func_node
                .into_iter()
                .filter(|x| x.kind() == "command_name")
                .filter_map(|x| cmds.get(x.as_str()).map(|funcs| (x, funcs)))
                .filter_map(|(x, funcs)| x.parent().map(|node| (x.to_string(), node, funcs)))
            {
                for f in funcs {
                    f(&cmd, &func_node, &cmd_node, &mut cursor, pkg, run);
                }
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
