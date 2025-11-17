use std::collections::HashMap;

use pkgcraft::bash::Node;
use pkgcraft::eapi::{EAPIS, Eapi};
use pkgcraft::pkg::{Package, ebuild::EbuildRawPkg};
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::VariableScopeInvalid;
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    kind: super::CheckKind::Variables,
    reports: &[VariableScopeInvalid],
    scope: Scope::Version,
    sources: &[SourceKind::EbuildRawPkg],
    context: &[],
    create,
}

type VariableFn = for<'a> fn(&str, &Node<'a>, &EbuildRawPkg, &ScannerRun);

pub(super) fn create(_run: &ScannerRun) -> super::Runner {
    let mut check = Check { variables: Default::default() };

    for eapi in &*EAPIS {
        check.register_eapi(eapi, eapi.env(), eapi_variable);
    }

    Box::new(check)
}

struct Check {
    variables: HashMap<&'static Eapi, HashMap<String, Vec<VariableFn>>>,
}

impl Check {
    /// Register EAPI variables for the check to handle.
    fn register_eapi<I>(&mut self, eapi: &'static Eapi, variables: I, func: VariableFn)
    where
        I: IntoIterator,
        I::Item: std::fmt::Display,
    {
        for variable in variables {
            self.variables
                .entry(eapi)
                .or_default()
                .entry(variable.to_string())
                .or_default()
                .push(func);
        }
    }
}

// TODO: handle nested function calls
/// Flag issues with EAPI variable usage.
fn eapi_variable(var: &str, var_node: &Node, pkg: &EbuildRawPkg, run: &ScannerRun) {
    let eapi_var = pkg.eapi().env().get(var).unwrap();
    if let Ok(Some(scope)) = var_node.in_scope()
        && !eapi_var.is_allowed(&scope)
    {
        VariableScopeInvalid
            .version(pkg)
            .message(format!("{var}: disabled in {scope} scope"))
            .location(var_node)
            .report(run);
    }
}

impl super::CheckRun for Check {
    fn run_ebuild_raw_pkg(&self, pkg: &EbuildRawPkg, run: &ScannerRun) {
        let eapi = pkg.eapi();
        let vars = self
            .variables
            .get(eapi)
            .unwrap_or_else(|| panic!("{pkg}: no variables registered for EAPI {eapi}"));

        for (var, var_node, funcs) in pkg
            .tree()
            .into_iter()
            .filter(|x| x.kind() == "variable_name")
            .filter_map(|x| vars.get(x.as_str()).map(|funcs| (x, funcs)))
            .filter_map(|(x, funcs)| x.parent().map(|node| (x.to_string(), node, funcs)))
        {
            for f in funcs {
                f(&var, &var_node, pkg, run);
            }
        }
    }
}
