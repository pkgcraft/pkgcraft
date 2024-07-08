use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;

use crate::report::ReportKind::PropertiesInvalid;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Properties,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[PropertiesInvalid],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static Repo) -> impl EbuildPkgCheck {
    Check {
        allowed: repo
            .trees()
            .flat_map(|x| x.metadata.config.properties_allowed.clone())
            .collect(),
    }
}

struct Check {
    allowed: HashSet<String>,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        if !self.allowed.is_empty() {
            let vals = pkg
                .properties()
                .iter_flatten()
                .filter(|x| !self.allowed.contains(x.as_str()))
                .collect::<HashSet<_>>();

            if !vals.is_empty() {
                let vals = vals.iter().sorted().join(", ");
                PropertiesInvalid
                    .version(pkg)
                    .message(format!("PROPERTIES not allowed: {vals}"))
                    .report(filter);
            }
        }
        // TODO: verify USE flags in conditionals
    }
}
