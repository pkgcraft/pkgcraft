use pkgcraft::error::Error::InvalidFetchable;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::traits::Contains;

use crate::report::ReportKind::UriInvalid;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::SrcUri,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[UriInvalid],
    context: &[],
};

pub(super) fn create() -> impl EbuildPkgCheck {
    Check
}

struct Check;

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        if !pkg.restrict().contains("fetch") {
            for result in pkg.fetchables() {
                match result {
                    Ok(_) => (),
                    Err(InvalidFetchable(e)) => UriInvalid.version(pkg).message(e).report(filter),
                    Err(e) => unreachable!("{pkg}: unhandled fetchable error: {e}"),
                }
            }
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
