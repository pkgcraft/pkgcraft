use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::{EbuildPackage, Pkg};
use pkgcraft::repo::{ebuild::Repo, PkgRepository};

use crate::report::{Report, ReportKind::DependencySlotMissing};
use crate::scope::Scope;
use crate::source::SourceKind;

pub(super) static CHECK: super::CheckInfo = super::CheckInfo {
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[DependencySlotMissing],
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
        for dep in pkg
            .rdepend()
            .intersection(pkg.depend())
            .flat_map(|x| x.iter_flatten())
            .filter(|x| x.blocker().is_none() && x.slot_dep().is_none())
        {
            // TODO: use cached lookup instead of searching for each dep
            let slots = self
                .repo
                .iter_restrict(dep.no_use_deps().as_ref())
                .map(|pkg| pkg.slot().to_string())
                .collect::<IndexSet<_>>();
            if slots.len() > 1 {
                let message = format!("{dep} matches multiple slots: {}", slots.iter().join(", "));
                report(DependencySlotMissing.version(pkg, message));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{TEST_DATA, TEST_DATA_PATCHED};
    use pretty_assertions::assert_eq;

    use crate::check::CheckKind::DependencySlotMissing;
    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    #[test]
    fn check() {
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let check_dir = repo.path().join(DependencySlotMissing);
        let scanner = Scanner::new().jobs(1).checks([DependencySlotMissing]);
        let expected = glob_reports!("{check_dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
