use indexmap::IndexSet;
use pkgcraft::pkg::ebuild::raw::Pkg;

use crate::bash::Tree;
use crate::report::ReportKind::VariableOrder;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, ParsedVersionCheck};

pub(crate) static CHECK: super::Check = super::Check {
    kind: CheckKind::VariableOrder,
    scope: Scope::Version,
    source: SourceKind::EbuildParsed,
    reports: &[VariableOrder],
    context: &[],
    priority: 0,
};

pub(crate) fn create() -> impl ParsedVersionCheck {
    // TODO: replace string variables with enum variants from pkgcraft?
    Check {
        ordered: [
            "DESCRIPTION",
            "HOMEPAGE",
            "SRC_URI",
            "S",
            "LICENSE",
            "SLOT",
            "KEYWORDS",
            "IUSE",
            "RESTRICT",
            "PROPERTIES",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
    }
}

struct Check {
    ordered: IndexSet<String>,
}

super::register!(Check);

impl ParsedVersionCheck for Check {
    fn run(&self, pkg: &Pkg, tree: &Tree, filter: &mut ReportFilter) {
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

            let name = tree.node_name(node);
            if let Some(idx) = self.ordered.get_index_of(name) {
                variables.push((idx, name, node.start_position().row + 1));
            }
        }

        let mut prev_idx = 0;
        for (idx, name, lineno) in variables {
            if idx < prev_idx {
                let unordered = self.ordered.get_index(prev_idx).unwrap();
                let message = format!("{name} should occur before {unordered}");
                filter.report(VariableOrder.version(pkg, message).line(lineno));
            }
            prev_idx = idx;
        }
    }
}
