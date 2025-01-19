use std::collections::{HashMap, HashSet};

use dashmap::DashMap;
use indexmap::IndexMap;
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::error::Error::UnversionedPkg;
use pkgcraft::pkg::ebuild::manifest::{HashType, ManifestType};
use pkgcraft::pkg::ebuild::EbuildPkg;

use crate::report::ReportKind::{ManifestCollide, ManifestConflict, ManifestInvalid};
use crate::scan::ScannerRun;

use super::EbuildPkgSetCheck;

pub(super) fn create(run: &ScannerRun) -> impl EbuildPkgSetCheck {
    Check {
        thin_manifests: run.repo.metadata().config.thin_manifests,
        colliding: Default::default(),
        conflicting: Default::default(),
        hash: run
            .repo
            .metadata()
            .config
            .manifest_required_hashes
            .iter()
            .next()
            .copied()
            .unwrap_or(HashType::Blake2b),
    }
}

static CHECK: super::Check = super::Check::Manifest;

struct Check {
    thin_manifests: bool,
    colliding: DashMap<String, HashMap<String, HashSet<Cpn>>>,
    conflicting: DashMap<String, HashMap<String, Cpn>>,
    hash: HashType,
}

super::register!(Check);

impl Check {
    // TODO: support inherited ignore directives from eclasses?
    /// Ignore ManifestMatch for go modules since go.mod files are designed to collide.
    fn is_go_module(&self, pkgs: &[EbuildPkg]) -> bool {
        pkgs.iter().any(|x| x.inherit().contains("go-module"))
    }
}

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], run: &ScannerRun) {
        // parse manifest
        let result = run.repo.metadata().pkg_manifest_parse(cpn);
        let Ok(manifest) = result else {
            if let Err(UnversionedPkg { err, .. }) = result {
                ManifestInvalid.package(cpn).message(err).report(run);
            }
            return;
        };

        // determine distfiles for pkgs and manifest
        let pkg_distfiles: HashSet<_> = pkgs.iter().flat_map(|p| p.distfiles()).collect();
        let mut manifest_distfiles = HashSet::new();
        let mut colliding = HashMap::<_, HashSet<_>>::new();
        for entry in manifest.distfiles() {
            let name = entry.name();
            manifest_distfiles.insert(name);

            if let Some(hash) = entry.hashes().get(&self.hash) {
                // track duplicate names with different hashes
                if run.enabled(ManifestConflict) {
                    self.conflicting
                        .entry(name.to_string())
                        .or_default()
                        .insert(hash.clone(), cpn.clone());
                }

                // track duplicate hashes with different names
                if run.enabled(ManifestCollide) && !self.is_go_module(pkgs) {
                    colliding
                        .entry(hash.clone())
                        .or_default()
                        .insert(name.to_string());
                }
            }
        }

        for (hash, files) in colliding {
            if files.len() > 1 {
                let files = files.iter().sorted().join(", ");
                ManifestCollide.package(cpn).message(files).report(run);
            }

            for file in files {
                self.colliding
                    .entry(hash.clone())
                    .or_default()
                    .entry(file)
                    .or_default()
                    .insert(cpn.clone());
            }
        }

        // flag manifest entries that don't match pkg distfiles
        let unknown = manifest_distfiles
            .difference(&pkg_distfiles)
            .sorted()
            .join(", ");
        if !unknown.is_empty() {
            ManifestInvalid
                .package(cpn)
                .message(format!("unknown: {unknown}"))
                .report(run);
        }

        // flag pkg distfiles that don't have manifest entries
        let missing = pkg_distfiles
            .difference(&manifest_distfiles)
            .sorted()
            .join(", ");
        if !missing.is_empty() {
            ManifestInvalid
                .package(cpn)
                .message(format!("missing: {missing}"))
                .report(run);
        }

        // flag non-distfile manifest entries for thin manifests
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
                    .report(run);
            }
        }
    }

    fn finish_check(&self, run: &ScannerRun) {
        if run.enabled(ManifestConflict) {
            for entry in self.conflicting.iter().filter(|x| x.len() > 1) {
                let (name, map) = entry.pair();
                let pkgs = map.values().sorted().join(", ");
                ManifestConflict
                    .repo(&run.repo)
                    .message(format!("{name}: {pkgs}"))
                    .report(run);
            }
        }

        if run.enabled(ManifestCollide) {
            for entry in self.colliding.iter().filter(|x| x.len() > 1) {
                // sort colliding entries by Cpn
                let mut map = IndexMap::<_, Vec<_>>::new();
                for (file, cpns) in entry.iter() {
                    for cpn in cpns {
                        map.entry(cpn).or_default().push(file);
                    }
                }

                // skip single package variants that are reported when running
                if map.len() > 1 {
                    map.sort_keys();
                    let values = map
                        .iter()
                        .map(|(cpn, files)| {
                            let files = files.iter().sorted().join(", ");
                            format!("({cpn}: {files})")
                        })
                        .join(", ");
                    ManifestCollide.repo(&run.repo).message(values).report(run);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;

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
        let scanner = Scanner::new().reports([CHECK]);
        let expected = glob_reports!("{dir}/**/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // verify ManifestCollide triggers in package scope
        let dir = dir.join("ManifestCollide");
        env::set_current_dir(&dir).unwrap();
        let expected = glob_reports!("{dir}/reports.json");
        let scanner = Scanner::new().reports([ManifestCollide]);
        let reports = scanner.run(repo, &dir).unwrap();
        assert_unordered_eq!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let scanner = Scanner::new().reports([CHECK]);
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
