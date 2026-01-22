use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::pkg::ebuild::keyword::KeywordStatus::Disabled;
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::KeywordsDropped;
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    kind: super::CheckKind::KeywordsDropped,
    reports: &[KeywordsDropped],
    scope: Scope::Package,
    sources: &[SourceKind::EbuildPkg],
    context: &[],
    create,
}

pub(super) fn create(_run: &ScannerRun) -> crate::Result<super::Runner> {
    Ok(Box::new(Check))
}

struct Check;

impl super::CheckRun for Check {
    fn run_ebuild_pkg_set(&self, _cpn: &Cpn, pkgs: &[EbuildPkg], run: &ScannerRun) {
        let mut seen = HashSet::new();
        let mut previous = HashSet::new();
        let mut changes = HashMap::<_, _>::new();

        for pkg in pkgs {
            // skip packages without keywords
            if pkg.keywords().is_empty() {
                continue;
            }

            let arches = pkg
                .keywords()
                .iter()
                .map(|k| k.arch())
                .collect::<HashSet<_>>();

            // globbed arches override all dropped keywords
            let drops = if arches.contains("*") {
                HashSet::new()
            } else {
                previous
                    .difference(&arches)
                    .chain(seen.difference(&arches))
                    .copied()
                    .collect()
            };

            for arch in drops {
                if run.repo.arches().contains(arch) {
                    changes.insert(arch.clone(), pkg);
                }
            }

            // ignore missing arches on previous versions that were re-enabled
            if !changes.is_empty() {
                let disabled = pkg
                    .keywords()
                    .iter()
                    .filter(|k| k.status() == Disabled)
                    .map(|k| k.arch())
                    .collect::<HashSet<_>>();
                let adds = arches
                    .difference(&previous)
                    .copied()
                    .collect::<HashSet<_>>();
                for arch in adds.difference(&disabled) {
                    changes.remove(*arch);
                }
            }

            seen.extend(arches.clone());
            previous = arches;
        }

        #[allow(clippy::mutable_key_type)]
        // false positive due to ebuild pkg OnceLock usage
        let mut dropped = HashMap::<_, Vec<_>>::new();
        for (arch, pkg) in changes {
            // TODO: report all pkgs with dropped keywords in verbose mode?
            // only report the latest pkg with dropped keywords
            dropped.entry(pkg).or_default().push(arch);
        }

        for (pkg, arches) in dropped {
            KeywordsDropped
                .version(pkg)
                .message(arches.iter().sorted().join(", "))
                .report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::{test_data, test_data_patched};

    use crate::scan::Scanner;
    use crate::test::{assert_unordered_reports, glob_reports};

    use super::*;

    #[test]
    fn check() {
        let scanner = Scanner::new().reports([CHECK]);

        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
