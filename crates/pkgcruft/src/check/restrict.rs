use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;

use crate::report::ReportKind::RestrictInvalid;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Restrict,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[RestrictInvalid],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static Repo) -> impl EbuildPkgCheck {
    Check {
        allowed: repo
            .trees()
            .flat_map(|x| x.metadata.config.restrict_allowed.clone())
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
            for val in pkg
                .restrict()
                .iter_flatten()
                .unique()
                .filter(|x| !self.allowed.contains(x.as_str()))
            {
                RestrictInvalid
                    .version(pkg)
                    .message(format!("RESTRICT not allowed: {val}"))
                    .report(filter);
            }
        }
        // TODO: verify USE flags in conditionals
    }
}
