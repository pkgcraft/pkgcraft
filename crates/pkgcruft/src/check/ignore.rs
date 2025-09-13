use itertools::Itertools;
use pkgcraft::dep::{Cpn, Cpv};

use crate::report::{ReportKind::IgnoreUnused, ReportScope};
use crate::scan::ScannerRun;

use super::{CategoryCheck, CpnCheck, CpvCheck};

pub(super) struct Check;

super::register!(Check, super::Check::Ignore);

impl CpvCheck for Check {
    fn finish_target(&self, cpv: &Cpv, run: &ScannerRun) {
        let scope = ReportScope::Version(cpv.clone(), None);

        // forciby populate the cache
        run.ignore.generate(&scope, Some(run)).count();

        // flag unused version scope ignore directives
        if let Some(sets) = run.ignore.unused(&scope) {
            let sets = sets.iter().join(", ");
            IgnoreUnused.version(cpv).message(sets).report(run);
        }
    }

    fn finish_check(&self, run: &ScannerRun) {
        let scope = ReportScope::Repo(run.repo.to_string());
        if let Some(sets) = run.ignore.unused(&scope) {
            let sets = sets.iter().join(", ");
            IgnoreUnused.repo(&run.repo).message(sets).report(run);
        }
    }
}

impl CpnCheck for Check {
    fn finish_target(&self, cpn: &Cpn, run: &ScannerRun) {
        let scope = ReportScope::Package(cpn.clone());

        // forciby populate the cache
        run.ignore.generate(&scope, Some(run)).count();

        // flag unused package scope ignore directives
        if let Some(sets) = run.ignore.unused(&scope) {
            let sets = sets.iter().join(", ");
            IgnoreUnused.package(cpn).message(sets).report(run);
        }
    }
}

impl CategoryCheck for Check {
    fn finish_target(&self, category: &str, run: &ScannerRun) {
        let scope = ReportScope::Category(category.to_string());

        // forciby populate the cache
        run.ignore.generate(&scope, Some(run)).count();

        if let Some(sets) = run.ignore.unused(&scope) {
            let sets = sets.iter().join(", ");
            IgnoreUnused.category(category).message(sets).report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use pkgcraft::test::test_data;

    use crate::report::ReportSet;
    use crate::scan::Scanner;
    use crate::test::{assert_ordered_reports, glob_reports};

    use super::*;

    #[test]
    fn check() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let invalid = glob_reports!("{dir}/IgnoreInvalid.json");
        let unused = glob_reports!("{dir}/IgnoreUnused.json");
        let expected: Vec<_> = invalid.into_iter().interleave(unused).collect();

        // check isn't run by default
        let scanner = Scanner::new();
        let mut reports = scanner.run(repo, repo).unwrap();
        assert!(!reports.any(|r| CHECK.reports().contains(&r.kind)));

        // return all report variants in order to trigger ignore filtering
        let scanner = Scanner::new().reports([ReportSet::All]);

        // version scope
        let reports: Vec<_> = scanner
            .run(repo, "Ignore/IgnoreUnused-0")
            .unwrap()
            .collect();
        assert_ordered_reports!(&reports, &expected[..2]);

        // package scope
        let reports: Vec<_> = scanner.run(repo, "Ignore/IgnoreUnused").unwrap().collect();
        assert_ordered_reports!(&reports, &expected[..4]);

        // category scope
        let reports: Vec<_> = scanner.run(repo, "Ignore/*").unwrap().collect();
        assert_ordered_reports!(&reports, &expected[..6]);

        // repo scope
        let reports: Vec<_> = scanner
            .run(repo, repo)
            .unwrap()
            .filter(|x| CHECK.reports().contains(&x.kind))
            .collect();
        assert_ordered_reports!(&reports, &expected);
    }
}
