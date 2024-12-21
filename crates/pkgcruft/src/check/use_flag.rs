use indexmap::IndexSet;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::report::ReportKind::UseFlagInvalid;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::UseFlag,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[UseFlagInvalid],
    context: &[],
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgCheck {
    Check {
        repo,
        use_expand: ["cpu_flags_"].into_iter().map(Into::into).collect(),
    }
}

struct Check {
    repo: &'static EbuildRepo,
    use_expand: IndexSet<String>,
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        for x in pkg.iuse() {
            if x.is_disabled() {
                UseFlagInvalid
                    .version(pkg)
                    .message(format!("disabled default: {x}"))
                    .report(filter);
            } else if x.is_enabled() && self.use_expand.iter().any(|s| x.flag().starts_with(s)) {
                UseFlagInvalid
                    .version(pkg)
                    .message(format!("enabled default: {x}"))
                    .report(filter);
            }
        }
    }
}
