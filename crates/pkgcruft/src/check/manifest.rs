use std::collections::{HashMap, HashSet};

use dashmap::DashMap;
use indexmap::IndexMap;
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::error::Error::UnversionedPkg;
use pkgcraft::pkg::ebuild::manifest::{HashType, ManifestType};
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::restrict::Scope;

use crate::iter::ReportFilter;
use crate::report::ReportKind::{ManifestCollide, ManifestConflict, ManifestInvalid};
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgSetCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Manifest,
    scope: Scope::Package,
    source: SourceKind::EbuildPkg,
    reports: &[ManifestInvalid, ManifestConflict, ManifestCollide],
    context: &[],
};

pub(super) fn create(repo: &EbuildRepo) -> impl EbuildPkgSetCheck {
    Check {
        repo: repo.clone(),
        thin_manifests: repo.metadata().config.thin_manifests,
        colliding: Default::default(),
        conflicting: Default::default(),
        hash: repo
            .metadata()
            .config
            .manifest_required_hashes
            .iter()
            .next()
            .copied()
            .unwrap_or(HashType::Blake2b),
    }
}

struct Check {
    repo: EbuildRepo,
    thin_manifests: bool,
    colliding: DashMap<String, HashMap<String, HashSet<Cpn>>>,
    conflicting: DashMap<String, HashMap<String, Cpn>>,
    hash: HashType,
}

super::register!(Check);

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], filter: &ReportFilter) {
        let manifest = match self.repo.metadata().pkg_manifest_parse(cpn) {
            Ok(value) => value,
            Err(UnversionedPkg { err, .. }) => {
                ManifestInvalid.package(cpn).message(err).report(filter);
                return;
            }
            Err(e) => unreachable!("{cpn}: unhandled manifest error: {e}"),
        };

        let mut manifest_distfiles = HashSet::new();
        for x in manifest.distfiles() {
            let name = x.name();
            manifest_distfiles.insert(name);

            if let Some(hash) = x.hashes().get(&self.hash) {
                // track duplicate names with different hashes
                if filter.enabled(ManifestConflict) {
                    self.conflicting
                        .entry(name.to_string())
                        .or_default()
                        .insert(hash.clone(), cpn.clone());
                }

                // track duplicate hashes with different names
                if filter.enabled(ManifestCollide) {
                    self.colliding
                        .entry(hash.clone())
                        .or_default()
                        .entry(name.to_string())
                        .or_default()
                        .insert(cpn.clone());
                }
            }
        }
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
                    .message(format!("unneeded: {files}"))
                    .report(filter);
            }
        }
    }

    fn finish(&self, repo: &EbuildRepo, filter: &ReportFilter) {
        if filter.enabled(ManifestConflict) {
            for entry in self.conflicting.iter().filter(|x| x.len() > 1) {
                let (name, map) = entry.pair();
                let pkgs = map.values().sorted().join(", ");
                ManifestConflict
                    .repo(repo)
                    .message(format!("{name}: {pkgs}"))
                    .report(filter);
            }
        }

        if filter.enabled(ManifestCollide) {
            for entry in self.colliding.iter().filter(|x| x.len() > 1) {
                // sort colliding entries by Cpn
                let mut map = IndexMap::<_, Vec<_>>::new();
                for (file, cpns) in entry.iter() {
                    for cpn in cpns {
                        map.entry(cpn).or_default().push(file);
                    }
                }
                map.sort_keys();

                let values = map
                    .iter()
                    .map(|(cpn, files)| {
                        let files = files.iter().sorted().join(", ");
                        format!("({cpn}: {files})")
                    })
                    .join(", ");
                ManifestCollide.repo(repo).message(values).report(filter);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::scan::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/**/reports.json");
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
