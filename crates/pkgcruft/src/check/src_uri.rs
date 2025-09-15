use dashmap::DashSet;
use itertools::Itertools;
use pkgcraft::error::Error;
use pkgcraft::fetch::Fetchable;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::{MirrorsUnused, UriInvalid};
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    super::Check {
        kind: super::CheckKind::SrcUri,
        reports: &[MirrorsUnused, UriInvalid],
        scope: Scope::Version,
        sources: &[SourceKind::EbuildPkg],
        context: &[],
        create,
    }
}

pub(super) fn create(run: &ScannerRun) -> super::Runner {
    let unused = if run.enabled(MirrorsUnused) {
        run.repo.metadata().mirrors().keys().cloned().collect()
    } else {
        Default::default()
    };

    Box::new(Check { unused })
}

struct Check {
    unused: DashSet<String>,
}

impl super::CheckRun for Check {
    fn run_ebuild_pkg(&self, pkg: &EbuildPkg, run: &ScannerRun) {
        for uri in pkg.src_uri().iter_flatten() {
            let result = Fetchable::from_uri(uri, pkg, false);
            let Ok(fetchable) = result else {
                if let Err(Error::InvalidFetchable(err)) = result {
                    UriInvalid.version(pkg).message(err).report(run);
                }
                continue;
            };

            if let Some(mirror) = fetchable.mirrors().first() {
                // mangle values for post-run finalization
                if run.enabled(MirrorsUnused) {
                    self.unused.remove(mirror.name());
                }
            }
        }
    }

    fn finish(&self, run: &ScannerRun) {
        if run.enabled(MirrorsUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            MirrorsUnused.repo(&run.repo).message(unused).report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::{assert_err_re, test_data, test_data_patched};

    use crate::scan::Scanner;
    use crate::test::{assert_unordered_reports, glob_reports};

    use super::*;

    #[test]
    fn check() {
        let scanner = Scanner::new().reports([CHECK]);

        // MirrorsUnused requires repo scope
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let r = scanner.run(repo, "SrcUri");
        assert_err_re!(r, "MirrorsUnused: report requires repo scope");

        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let expected = glob_reports!("{dir}/**/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
