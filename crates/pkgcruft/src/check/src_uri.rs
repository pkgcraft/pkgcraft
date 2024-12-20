use dashmap::DashSet;
use itertools::Itertools;
use pkgcraft::error::Error;
use pkgcraft::fetch::Fetchable;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::traits::Contains;

use crate::report::ReportKind::{MirrorsUnused, UriInvalid};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::SrcUri,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[MirrorsUnused, UriInvalid],
    context: &[],
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgCheck {
    Check {
        unused: repo.metadata().mirrors().keys().collect(),
    }
}

struct Check {
    unused: DashSet<&'static String>,
}

impl Check {
    fn process_fetchable(&self, fetchable: &Fetchable, filter: &mut ReportFilter) {
        if let Some((name, _)) = fetchable.mirrors() {
            // mangle values for post-run finalization
            if filter.finalize(MirrorsUnused) && !self.unused.is_empty() {
                self.unused.remove(name);
            }
        }
    }
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        if !pkg.restrict().contains("fetch") {
            for result in pkg.fetchables() {
                match result {
                    Ok(f) => self.process_fetchable(&f, filter),
                    // TODO: use deref patterns to matched boxed field when stabilized
                    // https://github.com/rust-lang/rust/issues/87121
                    Err(Error::Pkg { err, .. }) if matches!(*err, Error::InvalidFetchable(_)) => {
                        let Error::InvalidFetchable(error) = *err else {
                            panic!("invalid fetchable error");
                        };
                        UriInvalid.version(pkg).message(error).report(filter)
                    }
                    Err(e) => unreachable!("{pkg}: unhandled fetchable error: {e}"),
                }
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
            MirrorsUnused.repo(repo).message(unused).report(filter);
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
