use dashmap::DashSet;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::restrict::Scope;

use crate::report::ReportKind::{IuseInvalid, UseGlobalUnused};
use crate::scan::ScannerRun;
use crate::source::SourceKind;

super::register! {
    super::Check {
        kind: super::CheckKind::Iuse,
        reports: &[IuseInvalid, UseGlobalUnused],
        scope: Scope::Version,
        sources: &[SourceKind::EbuildPkg],
        context: &[],
        create,
    }
}

pub(super) fn create(run: &ScannerRun) -> super::Runner {
    let unused = if run.enabled(UseGlobalUnused) {
        run.repo
            .metadata()
            .use_global()
            .keys()
            .map(Into::into)
            .collect()
    } else {
        Default::default()
    };

    Box::new(Check {
        use_expand: ["cpu_flags_"].into_iter().map(Into::into).collect(),
        unused,
    })
}

struct Check {
    use_expand: IndexSet<String>,
    unused: DashSet<String>,
}

impl Check {
    /// Return true if an IUSE flag starts with any from a set.
    fn use_expand(&self, iuse: &str) -> bool {
        self.use_expand.iter().any(|s| iuse.starts_with(s))
    }
}

impl super::CheckRun for Check {
    fn run_ebuild_pkg(&self, pkg: &EbuildPkg, run: &ScannerRun) {
        for x in pkg.iuse() {
            if x.is_disabled() || (x.is_enabled() && self.use_expand(x.flag())) {
                IuseInvalid
                    .version(pkg)
                    .message(format!("invalid default: {x}"))
                    .report(run);
            }

            // mangle values for post-run finalization
            if run.enabled(UseGlobalUnused) {
                self.unused.remove(x.flag());
            }
        }
    }

    fn finish_check(&self, run: &ScannerRun) {
        if run.enabled(UseGlobalUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            UseGlobalUnused.repo(&run.repo).message(unused).report(run);
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
        let expected = glob_reports!("{dir}/**/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
