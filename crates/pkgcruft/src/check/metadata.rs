use pkgcraft::dep::Cpv;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::shell::pool::MetadataTaskBuilder;

use crate::report::ReportKind::MetadataError;
use crate::scan::ScannerRun;

use super::CpvCheck;

pub(super) fn create(run: &ScannerRun) -> impl CpvCheck + 'static {
    Check {
        regen: run.repo.pool().metadata_task(&run.repo),
    }
}

struct Check {
    regen: MetadataTaskBuilder,
}

super::register!(Check, super::Check::Metadata);

impl CpvCheck for Check {
    fn run(&self, cpv: &Cpv, run: &ScannerRun) {
        if let Err(InvalidPkg { err, .. }) = self.regen.run(cpv) {
            MetadataError.version(cpv).message(err).report(run)
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
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
