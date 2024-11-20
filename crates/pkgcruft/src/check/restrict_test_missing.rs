use pkgcraft::dep::{Dependency, DependencySet};
use pkgcraft::pkg::ebuild::iuse::Iuse;
use pkgcraft::pkg::ebuild::EbuildPkg;

use crate::report::ReportKind::RestrictMissing;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::RestrictTestMissing,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[RestrictMissing],
    context: &[],
    priority: 0,
};

pub(super) fn create() -> impl EbuildPkgCheck {
    Check {
        restricts: ["test", "!test? ( test )"]
            .iter()
            .map(|s| {
                Dependency::restrict(s).unwrap_or_else(|e| panic!("invalid RESTRICT: {s}: {e}"))
            })
            .collect(),
        iuse: Iuse::try_new("test").unwrap(),
    }
}

struct Check {
    restricts: DependencySet<String>,
    iuse: Iuse,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        if pkg.iuse().contains(&self.iuse)
            && pkg
                .restrict()
                .intersection(&self.restricts)
                .next()
                .is_none()
        {
            RestrictMissing
                .version(pkg)
                .message(r#"missing RESTRICT="!test? ( test )" with IUSE=test"#)
                .report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, test_data, test_data_patched};

    use crate::scanner::Scanner;
    use crate::test::*;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let data = test_data();
        let (pool, repo) = data.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(&pool).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let (pool, repo) = data.repo("qa-primary").unwrap();
        let scanner = Scanner::new(&pool).checks([CHECK]);
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, []);
    }
}
