use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;

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

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgCheck {
    Check {
        allowed: repo
            .trees()
            .flat_map(|r| r.metadata().config.restrict_allowed.clone())
            .collect(),
    }
}

struct Check {
    allowed: HashSet<String>,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        if !self.allowed.is_empty() {
            let vals = pkg
                .restrict()
                .iter_flatten()
                .filter(|x| !self.allowed.contains(x.as_str()))
                .collect::<HashSet<_>>();

            if !vals.is_empty() {
                let vals = vals.iter().sorted().join(", ");
                RestrictInvalid
                    .version(pkg)
                    .message(format!("RESTRICT not allowed: {vals}"))
                    .report(filter);
            }
        }
        // TODO: verify USE flags in conditionals
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, test_data_patched, TEST_DATA};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let (pool, repo) = TEST_DATA.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(&pool).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let (pool, repo) = data.repo("qa-primary").unwrap();
        let scanner = Scanner::new(&pool).checks([CHECK]);
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, []);
    }
}
