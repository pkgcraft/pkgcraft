use std::collections::{HashMap, HashSet};

use dashmap::DashMap;
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::error::Error::UnversionedPkg;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::pkg::ebuild::manifest::{DEFAULT_HASHES, HashType, ManifestType};
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::{ManifestCollide, ManifestConflict, ManifestInvalid};
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    kind: super::CheckKind::Manifest,
    reports: &[ManifestCollide, ManifestConflict, ManifestInvalid],
    scope: Scope::Package,
    sources: &[SourceKind::EbuildPkg],
    context: &[],
    create,
}

pub(super) fn create(run: &ScannerRun) -> super::Runner {
    let config = &run.repo.metadata().config;

    // determine default hashes
    let manifest_hashes = if config.manifest_hashes.is_empty() {
        DEFAULT_HASHES.iter().copied().collect()
    } else {
        config.manifest_hashes.iter().copied().collect()
    };

    // determine required hash
    let required_hash = if config.manifest_required_hashes.is_empty() {
        DEFAULT_HASHES.iter().next().copied().unwrap()
    } else {
        config
            .manifest_required_hashes
            .iter()
            .next()
            .copied()
            .unwrap()
    };

    Box::new(Check {
        thin_manifests: config.thin_manifests(),
        colliding: Default::default(),
        conflicting: Default::default(),
        manifest_hashes,
        required_hash,
    })
}

struct Check {
    thin_manifests: bool,
    colliding: DashMap<String, HashMap<String, HashSet<Cpn>>>,
    conflicting: DashMap<String, HashMap<String, Cpn>>,
    manifest_hashes: IndexSet<HashType>,
    required_hash: HashType,
}

impl Check {
    // TODO: support inherited ignore directives from eclasses?
    /// Ignore ManifestMatch for go modules since go.mod files are designed to collide.
    fn is_go_module(&self, pkgs: &[EbuildPkg]) -> bool {
        pkgs.iter().any(|x| x.inherit().contains("go-module"))
    }
}

impl super::CheckRun for Check {
    fn run_ebuild_pkg_set(&self, cpn: &Cpn, pkgs: &[EbuildPkg], run: &ScannerRun) {
        // parse manifest
        let manifest = match run.repo.metadata().pkg_manifest_parse(cpn) {
            Ok(manifest) => manifest,
            Err(UnversionedPkg { err, .. }) => {
                ManifestInvalid.package(cpn).message(err).report(run);
                return;
            }
            Err(e) => unreachable!("unexpected manifest parsing error: {e}"),
        };

        let pkg_distfiles: HashSet<_> = pkgs.iter().flat_map(|p| p.distfiles()).collect();
        let mut manifest_distfiles = HashSet::new();
        let mut colliding = HashMap::<_, HashSet<_>>::new();
        let mut unneeded = HashSet::new();

        // determine distfiles for pkgs and manifest
        for entry in &manifest {
            let name = entry.name();

            if entry.kind() == ManifestType::Dist {
                manifest_distfiles.insert(name);

                if let Some(hash) = entry.hashes().get(&self.required_hash) {
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
            } else if self.thin_manifests {
                unneeded.insert(name);
            }

            // flag missing hash variants
            let missing_hashes = self
                .manifest_hashes
                .iter()
                .filter(|&x| !entry.hashes().contains_key(x))
                .join(", ");
            if !missing_hashes.is_empty() {
                ManifestInvalid
                    .package(cpn)
                    .message(format!("{name}: missing hashes: {missing_hashes}"))
                    .report(run);
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

        // flag pkg distfiles with missing manifest entries
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
        let unneeded = unneeded.iter().sorted().join(", ");
        if !unneeded.is_empty() {
            ManifestInvalid
                .package(cpn)
                .message(format!("unneeded: {unneeded}"))
                .report(run);
        }
    }

    fn finish(&self, run: &ScannerRun) {
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

    use pkgcraft::test::{test_data, test_data_patched};

    use crate::scan::Scanner;
    use crate::test::{assert_unordered_reports, glob_reports};

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
        assert_unordered_reports!(reports, expected);

        // verify ManifestCollide triggers in package scope
        let dir = dir.join("ManifestCollide");
        env::set_current_dir(&dir).unwrap();
        let expected = glob_reports!("{dir}/reports.json");
        let scanner = Scanner::new().reports([ManifestCollide]);
        let reports = scanner.run(repo, &dir).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let scanner = Scanner::new().reports([CHECK]);
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
