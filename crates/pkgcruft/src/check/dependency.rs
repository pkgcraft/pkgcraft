use itertools::Itertools;
use pkgcraft::dep::{Dependency, Operator, SlotOperator, UseDepKind};
use pkgcraft::pkg::ebuild::{metadata::Key, EbuildPackage, EbuildPkg};
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::traits::Intersects;

use crate::report::ReportKind::{
    DependencyDeprecated, DependencyInvalid, DependencyRevisionMissing,
};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Dependency,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[DependencyDeprecated, DependencyInvalid, DependencyRevisionMissing],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgCheck {
    Check { repo }
}

struct Check {
    repo: &'static EbuildRepo,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        for key in pkg.eapi().dep_keys().iter().copied() {
            let deps = pkg.dependencies(&[key]);
            for dep in deps.iter_flatten().unique() {
                // verify conditional use deps map to IUSE flags
                for flag in dep
                    .use_deps()
                    .into_iter()
                    .flatten()
                    .filter(|x| matches!(x.kind(), UseDepKind::Conditional))
                    .map(|x| x.flag())
                    .filter(|flag| !pkg.iuse_effective().contains(*flag))
                {
                    DependencyInvalid
                        .version(pkg)
                        .message(format!("{key}: missing IUSE={flag}: {dep}"))
                        .report(filter);
                }

                if self.repo.deprecated(dep).is_some() {
                    // drop use deps since package.deprecated doesn't include them
                    DependencyDeprecated
                        .version(pkg)
                        .message(format!("{key}: {}", dep.no_use_deps()))
                        .report(filter);
                }

                // TODO: consider moving into parser when it supports dynamic error strings
                if dep.slot_op() == Some(SlotOperator::Equal) {
                    if dep.blocker().is_some() {
                        DependencyInvalid
                            .version(pkg)
                            .message(format!("{key}: = slot operator with blocker: {dep}"))
                            .report(filter);
                    }

                    if dep.subslot().is_some() {
                        DependencyInvalid
                            .version(pkg)
                            .message(format!("{key}: = slot operator with subslot: {dep}"))
                            .report(filter);
                    }

                    if key == Key::PDEPEND {
                        DependencyInvalid
                            .version(pkg)
                            .message(format!("{key}: = slot operator invalid: {dep}"))
                            .report(filter);
                    }
                }

                if dep.blocker().is_some() && dep.intersects(pkg) {
                    DependencyInvalid
                        .version(pkg)
                        .message(format!("{key}: blocker matches package: {dep}"))
                        .report(filter);
                }

                if dep.op() == Some(Operator::Equal) && dep.revision().is_none() {
                    DependencyRevisionMissing
                        .version(pkg)
                        .message(format!("{key}: {dep}"))
                        .report(filter);
                }
            }

            // TODO: consider moving into parser when it supports dynamic error strings
            for dep in deps
                .iter_recursive()
                .filter(|x| matches!(x, Dependency::AnyOf(_)))
                .flat_map(|x| x.iter_flatten())
                .filter(|x| x.slot_op() == Some(SlotOperator::Equal))
                .unique()
            {
                DependencyInvalid
                    .version(pkg)
                    .message(format!("{key}: = slot operator in any-of: {dep}"))
                    .report(filter);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, TEST_DATA, TEST_DATA_PATCHED};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
