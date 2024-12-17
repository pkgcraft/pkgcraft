use std::collections::HashMap;

use itertools::Itertools;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::report::ReportKind::{EbuildNameInvalid, EbuildVersionsEqual};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, CpnCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::EbuildName,
    scope: Scope::Package,
    source: SourceKind::Cpn,
    reports: &[EbuildNameInvalid, EbuildVersionsEqual],
    context: &[],
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl CpnCheck {
    Check { repo }
}

struct Check {
    repo: &'static EbuildRepo,
}

impl CpnCheck for Check {
    fn run(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        let mut cpvs = HashMap::<Cpv, Vec<_>>::new();

        for result in self.repo.cpvs_from_package(cpn.category(), cpn.package()) {
            match result {
                Err(e) => EbuildNameInvalid.package(cpn).message(e).report(filter),
                Ok(cpv) => {
                    let version = cpv.version().to_string();
                    cpvs.entry(cpv).or_default().push(version);
                }
            }
        }

        for versions in cpvs.values() {
            if versions.len() > 1 {
                let versions = versions.iter().sorted().join(", ");
                EbuildVersionsEqual
                    .package(cpn)
                    .message(format!("versions overlap: {versions}"))
                    .report(filter);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, test_data, test_data_patched};

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
