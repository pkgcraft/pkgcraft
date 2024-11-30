use pkgcraft::pkg::{ebuild::EbuildRawPkg, Package};
use pkgcraft::shell::commands::Command;
use std::collections::{HashMap, HashSet};

use crate::bash::Tree;
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

type TestFn = fn(&str, &HashSet<Command>) -> Option<String>;

fn find(cmd: &str, builtins: &HashSet<Command>) -> Option<String> {
    cmd.split_whitespace()
        .skip_while(|x| *x != "-exec")
        .nth(1)
        .and_then(|x| builtins.get(x))
        .map(|x| x.to_string())
}

fn xargs(cmd: &str, builtins: &HashSet<Command>) -> Option<String> {
    cmd.split_whitespace()
        .nth(1)
        .and_then(|x| builtins.get(x))
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
    fn run(&self, pkg: &EbuildRawPkg, tree: &Tree, filter: &mut ReportFilter) {
        // TODO: use parse tree query
        for node in tree.iter_func().filter(|x| x.kind() == "command_name") {
            let cmd_name = node.as_str();
            if let Some(func) = self.commands.get(cmd_name) {
                let builtins = pkg.eapi().commands();
                let cmd = node.parent().unwrap();
                if let Some(builtin) = func(cmd.as_str(), builtins) {
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
