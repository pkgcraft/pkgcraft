use dashmap::DashSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::Eclass;
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::EclassUnused;
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    super::Check {
        kind: super::CheckKind::Eclass,
        reports: &[EclassUnused],
        scope: Scope::Version,
        sources: &[SourceKind::EbuildPkg],
        context: &[],
        create,
    }
}

pub(super) fn create(run: &ScannerRun) -> super::Runner {
    let unused = if run.enabled(EclassUnused) {
        run.repo.metadata().eclasses().iter().cloned().collect()
    } else {
        Default::default()
    };

    Box::new(Check { unused })
}

struct Check {
    unused: DashSet<Eclass>,
}

impl super::CheckRun for Check {
    fn run_ebuild_pkg(&self, pkg: &EbuildPkg, _run: &ScannerRun) {
        for eclass in pkg.inherited() {
            self.unused.remove(eclass);
        }
    }

    fn finish(&self, run: &ScannerRun) {
        if run.enabled(EclassUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            EclassUnused.repo(&run.repo).message(unused).report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::{test_data, test_data_patched};

    use crate::scan::Scanner;
    use crate::test::{assert_unordered_reports, glob_reports};

    use super::*;

    #[test]
    fn check() {
        let scanner = Scanner::new().reports([CHECK]);

        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let expected = glob_reports!("{dir}/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
