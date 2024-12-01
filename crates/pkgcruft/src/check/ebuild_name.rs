use std::collections::HashSet;

use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::report::ReportKind::EbuildNameInvalid;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, CpnCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::EbuildName,
    scope: Scope::Package,
    source: SourceKind::Cpn,
    reports: &[EbuildNameInvalid],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl CpnCheck {
    Check { repo }
}

struct Check {
    repo: &'static EbuildRepo,
}

impl CpnCheck for Check {
    fn run(&self, cpn: &Cpn, filter: &mut ReportFilter) {
        let mut cpvs = HashSet::<Cpv>::new();
        for result in self.repo.cpvs_from_package(cpn.category(), cpn.package()) {
            match result {
                Err(e) => EbuildNameInvalid
                    .package(cpn)
                    .message(format!("{e}"))
                    .report(filter),
                Ok(cpv) => {
                    if let Some(existing) = cpvs.get(&cpv) {
                        EbuildNameInvalid
                            .version(cpv)
                            .message(format!("version overlaps: {}", existing.version()))
                            .report(filter);
                    } else {
                        cpvs.insert(cpv);
                    }
                }
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
