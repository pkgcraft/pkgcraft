use std::collections::HashSet;

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
        used_files: Default::default(),
        conflicting: Default::default(),
        used_hashes: Default::default(),
        colliding: Default::default(),
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
    used_files: DashMap<String, (Cpn, String)>,
    conflicting: DashMap<String, HashSet<Cpn>>,
    used_hashes: DashMap<String, (Cpn, String)>,
    colliding: DashMap<String, IndexMap<Cpn, HashSet<String>>>,
    hash: HashType,
}

impl Check {
    // TODO: Drop this once ignore file support is added?
    /// Ignore ManifestMatch for go modules since go.mod files are designed to collide.
    fn is_go_module(&self, pkgs: &[EbuildPkg]) -> bool {
        pkgs.iter().any(|x| x.inherit().contains("go-module"))
    }
}

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], filter: &mut ReportFilter) {
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

            if filter.enabled(ManifestConflict) || filter.enabled(ManifestCollide) {
                if let Some(hash) = x.hashes().get(&self.hash) {
                    // check for duplicate names with different hashes
                    if let Some(entry) = self.used_files.get(name) {
                        let (pkg, value) = entry.value();
                        if hash != value {
                            self.conflicting
                                .entry(name.to_string())
                                .or_default()
                                .extend([pkg.clone(), cpn.clone()]);
                        }
                    } else {
                        self.used_files
                            .insert(name.to_string(), (cpn.clone(), hash.clone()));
                    }

                    // check for duplicate hashes with different names
                    if !self.is_go_module(pkgs) {
                        if let Some(entry) = self.used_hashes.get(hash) {
                            let (pkg, file) = entry.value();
                            if name != file {
                                for (pkg, name) in [(pkg, file.as_str()), (cpn, name)] {
                                    self.colliding
                                        .entry(hash.clone())
                                        .or_default()
                                        .entry(pkg.clone())
                                        .or_default()
                                        .insert(name.to_string());
                                }
                            }
                        } else {
                            self.used_hashes
                                .insert(hash.clone(), (cpn.clone(), name.to_string()));
                        }
                    }
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

    fn finish(&self, repo: &EbuildRepo, filter: &mut ReportFilter) {
        for entry in &self.conflicting {
            let (name, pkgs) = entry.pair();
            let pkgs = pkgs.iter().sorted().join(", ");
            ManifestConflict
                .repo(repo)
                .message(format!("{name}: {pkgs}"))
                .report(filter);
        }

        for mut entry in self.colliding.iter_mut() {
            entry.sort_keys();
            let values = entry
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
