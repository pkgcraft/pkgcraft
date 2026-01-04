use pkgcraft::dep::Cpv;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::restrict::Scope;
use pkgcraft::shell::pool::MetadataTaskBuilder;

use crate::report::ReportKind::MetadataError;
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    kind: super::CheckKind::EbuildMetadata,
    reports: &[MetadataError],
    scope: Scope::Version,
    sources: &[SourceKind::Cpv],
    context: &[],
    create,
}

pub(super) fn create(run: &ScannerRun) -> super::Runner {
    Box::new(Check {
        regen: run.repo.pool().metadata_task(&run.repo),
    })
}

struct Check {
    regen: MetadataTaskBuilder,
}

impl super::CheckRun for Check {
    fn run_cpv(&self, cpv: &Cpv, run: &ScannerRun) {
        match self.regen.run(cpv) {
            Ok(_) => (),
            Err(InvalidPkg { err, .. }) => MetadataError.version(cpv).message(err).report(run),
            Err(e) => unreachable!("unexpected metadata error: {e}"),
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
