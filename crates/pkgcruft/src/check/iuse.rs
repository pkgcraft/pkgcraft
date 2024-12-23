use dashmap::DashSet;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::{IuseInvalid, UseGlobalUnused};
use crate::scanner::ReportFilter;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Iuse,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[IuseInvalid, UseGlobalUnused],
    context: &[],
};

pub(super) fn create(repo: &EbuildRepo) -> impl EbuildPkgCheck {
    Check {
        use_expand: ["cpu_flags_"].into_iter().map(Into::into).collect(),
        unused: repo
            .metadata()
            .use_global()
            .keys()
            .map(Into::into)
            .collect(),
    }
}

struct Check {
    use_expand: IndexSet<String>,
    unused: DashSet<String>,
}

impl Check {
    /// Return true if an IUSE flag starts with any from a set.
    fn use_expand(&self, iuse: &str) -> bool {
        self.use_expand.iter().any(|s| iuse.starts_with(s))
    }
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        for x in pkg.iuse() {
            if x.is_disabled() || (x.is_enabled() && self.use_expand(x.flag())) {
                IuseInvalid
                    .version(pkg)
                    .message(format!("invalid default: {x}"))
                    .report(filter);
            }

            // mangle values for post-run finalization
            if filter.finalize(UseGlobalUnused) {
                self.unused.remove(x.flag());
            }
        }
    }

    fn finish(&self, repo: &EbuildRepo, filter: &mut ReportFilter) {
        if !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            UseGlobalUnused.repo(repo).message(unused).report(filter);
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
