use dashmap::DashSet;
use itertools::Itertools;
use pkgcraft::error::Error;
use pkgcraft::fetch::Fetchable;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::restrict::Scope;

use crate::iter::ReportFilter;
use crate::report::ReportKind::{MirrorsUnused, UriInvalid};
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::SrcUri,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[MirrorsUnused, UriInvalid],
    context: &[],
};

pub(super) fn create(repo: &EbuildRepo, filter: &ReportFilter) -> impl EbuildPkgCheck {
    let unused = if filter.enabled(MirrorsUnused) {
        repo.metadata().mirrors().keys().cloned().collect()
    } else {
        Default::default()
    };

    Check { unused }
}

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

    fn finish(&self, repo: &EbuildRepo, filter: &ReportFilter) {
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
