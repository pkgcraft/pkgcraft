use dashmap::DashSet;
use itertools::Itertools;
use pkgcraft::error::Error;
use pkgcraft::fetch::Fetchable;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::iter::ReportFilter;
use crate::report::ReportKind::{MirrorsUnused, UriInvalid};
use crate::scan::ScannerRun;

use super::EbuildPkgCheck;

pub(super) fn create(run: &ScannerRun) -> impl EbuildPkgCheck {
    let unused = if run.enabled(MirrorsUnused) {
        run.repo.metadata().mirrors().keys().cloned().collect()
    } else {
        Default::default()
    };

    Check { unused }
}

static CHECK: super::Check = super::Check::SrcUri;

struct Check {
    unused: DashSet<String>,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &ReportFilter) {
        for uri in pkg.src_uri().iter_flatten() {
            let result = Fetchable::from_uri(uri, pkg, false);
            let Ok(fetchable) = result else {
                if let Err(Error::InvalidFetchable(err)) = result {
                    UriInvalid.version(pkg).message(err).report(filter);
                }
                continue;
            };

            if let Some(mirror) = fetchable.mirrors().first() {
                // mangle values for post-run finalization
                if filter.enabled(MirrorsUnused) {
                    self.unused.remove(mirror.name());
                }
            }
        }
    }

    fn finish_check(&self, repo: &EbuildRepo, filter: &ReportFilter) {
        if filter.enabled(MirrorsUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            MirrorsUnused.repo(repo).message(unused).report(filter);
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
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
