use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::manifest::ManifestType;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::report::ReportKind::ManifestInvalid;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgSetCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Manifest,
    scope: Scope::Package,
    source: SourceKind::EbuildPkg,
    reports: &[ManifestInvalid],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgSetCheck {
    Check {
        repo,
        thin_manifests: repo.metadata().config.thin_manifests,
    }
}

struct Check {
    repo: &'static EbuildRepo,
    thin_manifests: bool,
}

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], filter: &mut ReportFilter) {
        let manifest = match self.repo.metadata().pkg_manifest_parse(cpn) {
            Ok(value) => value,
            Err(e) => {
                ManifestInvalid.package(cpn).message(e).report(filter);
                return;
            }
        };

        let manifest_distfiles: HashSet<_> = manifest.distfiles().map(|x| x.name()).collect();
        let pkg_distfiles: HashSet<_> = pkgs.iter().flat_map(|p| p.distfiles()).collect();

        let unknown = manifest_distfiles
            .difference(&pkg_distfiles)
            .sorted()
            .join(", ");
        if !unknown.is_empty() {
            ManifestInvalid
                .package(cpn)
                .message(format!("unknown: {unknown}"))
                .report(filter);
        }

        let missing = pkg_distfiles
            .difference(&manifest_distfiles)
            .sorted()
            .join(", ");
        if !missing.is_empty() {
            ManifestInvalid
                .package(cpn)
                .message(format!("missing: {missing}"))
                .report(filter);
        }

        if self.thin_manifests {
            let files: HashSet<_> = manifest
                .iter()
                .filter(|x| x.kind() != ManifestType::Dist)
                .map(|x| x.name())
                .collect();
            if !files.is_empty() {
                let files = files.iter().sorted().join(", ");
                ManifestInvalid
                    .package(cpn)
                    .message(format!("unnecessary: {files}"))
                    .report(filter);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
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
