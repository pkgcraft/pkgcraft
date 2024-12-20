use dashmap::DashSet;
use itertools::Itertools;
use pkgcraft::dep::{Dep, Dependency, Operator, SlotOperator, UseDepKind};
use pkgcraft::pkg::ebuild::{metadata::Key, EbuildPkg};
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::traits::Intersects;

use crate::report::ReportKind::{
    DependencyDeprecated, DependencyInvalid, DependencyRevisionMissing, PackageDeprecatedUnused,
};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Dependency,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[
        DependencyDeprecated,
        DependencyInvalid,
        DependencyRevisionMissing,
        PackageDeprecatedUnused,
    ],
    context: &[],
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgCheck {
    Check {
        repo,
        unused: repo.metadata().pkg_deprecated().iter().collect(),
    }
}

struct Check {
    repo: &'static EbuildRepo,
    unused: DashSet<&'static Dep>,
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        for key in pkg.eapi().dep_keys().iter().copied() {
            let deps = pkg.dependencies([key]);
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

                if let Some(entry) = self.repo.deprecated(dep) {
                    // drop use deps since package.deprecated doesn't include them
                    DependencyDeprecated
                        .version(pkg)
                        .message(format!("{key}: {}", dep.no_use_deps()))
                        .report(filter);

                    // mangle values for post-run finalization
                    if filter.finish() && !self.unused.is_empty() {
                        self.unused.remove(entry);
                    }
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

    fn finish(&self, repo: &EbuildRepo, filter: &mut ReportFilter) {
        if !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            PackageDeprecatedUnused
                .repo(repo)
                .message(unused)
                .report(filter);
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
