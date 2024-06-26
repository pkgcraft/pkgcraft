use indexmap::IndexSet;
use pkgcraft::dep::{Flatten, Operator};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;

use crate::report::ReportKind::{DependencyDeprecated, DependencyRevisionMissing};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Dependency,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[DependencyDeprecated, DependencyRevisionMissing],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static Repo) -> impl EbuildPkgCheck {
    Check { repo }
}

struct Check {
    repo: &'static Repo,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        for key in pkg.eapi().dep_keys() {
            let deps = pkg
                .dependencies(&[*key])
                .into_iter_flatten()
                .collect::<IndexSet<_>>();

            for dep in deps {
                if self.repo.deprecated(dep).is_some() {
                    // drop use deps since package.deprecated doesn't include them
                    DependencyDeprecated
                        .version(pkg)
                        .message(format!("{key}: {}", dep.no_use_deps()))
                        .report(filter);
                }

                if matches!(dep.op(), Some(Operator::Equal)) && dep.revision().is_none() {
                    DependencyRevisionMissing
                        .version(pkg)
                        .message(format!("{key}: {dep}"))
                        .report(filter);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{TEST_DATA, TEST_DATA_PATCHED};
    use pretty_assertions::assert_eq;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
