use dashmap::DashSet;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::iter::ReportFilter;
use crate::report::ReportKind::{IuseInvalid, UseGlobalUnused};

use super::EbuildPkgCheck;

pub(super) fn create(repo: &EbuildRepo, filter: &ReportFilter) -> impl EbuildPkgCheck {
    let unused = if filter.enabled(UseGlobalUnused) {
        repo.metadata()
            .use_global()
            .keys()
            .map(Into::into)
            .collect()
    } else {
        Default::default()
    };

    Check {
        use_expand: ["cpu_flags_"].into_iter().map(Into::into).collect(),
        unused,
    }
}

static CHECK: super::Check = super::Check::Iuse;

struct Check {
    use_expand: IndexSet<String>,
    unused: DashSet<String>,
}

super::register!(Check);

impl Check {
    /// Return true if an IUSE flag starts with any from a set.
    fn use_expand(&self, iuse: &str) -> bool {
        self.use_expand.iter().any(|s| iuse.starts_with(s))
    }
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &ReportFilter) {
        for x in pkg.iuse() {
            if x.is_disabled() || (x.is_enabled() && self.use_expand(x.flag())) {
                IuseInvalid
                    .version(pkg)
                    .message(format!("invalid default: {x}"))
                    .report(filter);
            }

            // mangle values for post-run finalization
            if filter.enabled(UseGlobalUnused) {
                self.unused.remove(x.flag());
            }
        }
    }

    fn finish(&self, repo: &EbuildRepo, filter: &ReportFilter) {
        if filter.enabled(UseGlobalUnused) && !self.unused.is_empty() {
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

    use crate::scan::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/**/reports.json");
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
