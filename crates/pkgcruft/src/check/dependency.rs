use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::dep::{Flatten, Operator};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{
    Report,
    ReportKind::{DependencyDeprecated, RevisionMissing},
};
use crate::scope::Scope;
use crate::source::SourceKind;

pub(super) static CHECK: super::CheckInfo = super::CheckInfo {
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[DependencyDeprecated, RevisionMissing],
    context: &[],
    priority: 0,
};

#[derive(Debug)]
pub(crate) struct Check<'a> {
    repo: &'a Repo,
}

impl<'a> Check<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { repo }
    }
}

impl<'a> super::CheckRun<&Pkg<'a>> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkg: &Pkg<'a>, mut report: F) {
        for key in pkg.eapi().dep_keys() {
            let mut deprecated = HashSet::new();

            for dep in pkg.dependencies(&[*key]).into_iter_flatten() {
                if self.repo.deprecated(dep).is_some() {
                    // drop use deps since package.deprecated doesn't include them
                    deprecated.insert(dep.no_use_deps());
                }

                if matches!(dep.op(), Some(Operator::Equal)) && dep.revision().is_none() {
                    let message = format!("{key}: {dep}");
                    report(RevisionMissing.version(pkg, message));
                }
            }

            if !deprecated.is_empty() {
                let message = format!("{key}: {}", deprecated.iter().sorted().join(", "));
                report(DependencyDeprecated.version(pkg, message));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{TEST_DATA, TEST_DATA_PATCHED};
    use pretty_assertions::assert_eq;

    use crate::check::CheckKind::Dependency;
    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    #[test]
    fn check() {
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let check_dir = repo.path().join(Dependency);
        let scanner = Scanner::new().jobs(1).checks([Dependency]);
        let expected = glob_reports!("{check_dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
