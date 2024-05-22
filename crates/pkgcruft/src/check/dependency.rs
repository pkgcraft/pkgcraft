use std::collections::HashSet;

use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::{Flatten, Operator};
use pkgcraft::pkg::ebuild::{EbuildPackage, Pkg};
use pkgcraft::pkg::Package;
use pkgcraft::repo::{ebuild::Repo, PkgRepository};

use crate::report::{
    Report,
    ReportKind::{self, DeprecatedDependency, MissingRevision, MissingSlotDep},
};

pub(super) static REPORTS: &[ReportKind] = &[DeprecatedDependency, MissingRevision, MissingSlotDep];

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
                    report(MissingRevision.version(pkg, message));
                }
            }

            if !deprecated.is_empty() {
                let message = format!("{key}: {}", deprecated.iter().sorted().join(", "));
                report(DeprecatedDependency.version(pkg, message));
            }
        }

        for dep in pkg
            .rdepend()
            .intersection(pkg.depend())
            .flat_map(|x| x.iter_flatten())
        {
            if dep.blocker().is_none() && dep.slot_dep().is_none() {
                // TODO: use cached lookup instead of searching for each dep
                let slots: IndexSet<_> = self
                    .repo
                    .iter_restrict(dep.no_use_deps().as_ref())
                    .map(|pkg| pkg.slot().to_string())
                    .collect();
                if slots.len() > 1 {
                    let message =
                        format!("{dep} matches multiple slots: {}", slots.iter().join(", "));
                    report(MissingSlotDep.version(pkg, message));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;
    use pretty_assertions::assert_eq;

    use crate::check::CheckKind::Dependency;
    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    #[test]
    fn check() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let check_dir = repo.path().join(Dependency.as_ref());
        let scanner = Scanner::new().jobs(1).checks([Dependency]);
        let expected = glob_reports!("{check_dir}/*/reports.json");

        // check dir restriction
        let restrict = repo.restrict_from_path(&check_dir).unwrap();
        let reports: Vec<_> = scanner.run(repo, [&restrict]).collect();
        assert_eq!(&reports, &expected);

        // repo restriction
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);
    }
}
