use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::{pkgs_ebuild_raw, MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcraft::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use pkgcraft::pkg::Pretend;
use pkgcraft::repo::RepoFormat;
use pkgcraft::utils::bounded_jobs;
use scallop::pool::PoolIter;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Pretend options")]
pub(crate) struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    // positionals
    /// Target packages or paths
    #[arg(value_name = "TARGET", default_value = ".", help_heading = "Arguments")]
    targets: Vec<MaybeStdinVec<String>>,
}

// TODO: use configured ebuild repos instead of raw ones
// TODO: support binpkg repos
impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let func = |pkg: pkgcraft::Result<EbuildRawPkg>| -> scallop::Result<Option<String>> {
            let pkg = EbuildPkg::try_from(pkg?)?;
            pkg.pretend()
        };

        // loop over targets, tracking overall failure status
        let jobs = bounded_jobs(self.jobs.unwrap_or_default());
        let mut failed = false;

        // convert targets to restrictions
        let targets: Vec<_> = TargetRestrictions::new(config)
            .repo_format(RepoFormat::Ebuild)
            .targets(self.targets.iter().flatten())
            .try_collect()?;
        config.finalize()?;

        // convert restrictions to pkgs
        let pkgs = pkgs_ebuild_raw(targets);

        // run pkg_pretend across selected pkgs
        let (mut stdout, mut stderr) = (io::stdout().lock(), io::stderr().lock());
        for result in PoolIter::new(jobs, pkgs, func, true)? {
            match result {
                Err(e) => {
                    failed = true;
                    writeln!(stderr, "{e}")?;
                }
                Ok(Some(s)) => writeln!(stdout, "{s}")?,
                Ok(None) => (),
            }
        }

        Ok(ExitCode::from(failed as u8))
    }
}
