use std::io::{self, Write};
use std::process::ExitCode;

use clap::{Args, builder::ArgPredicate};
use pkgcraft::cli::{MaybeStdinVec, Targets};
use pkgcraft::config::Config;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::RepoFormat;
use pkgcraft::traits::{LogErrors, ParallelMapOrdered};

#[derive(Args)]
#[clap(next_help_heading = "Pretend options")]
pub(crate) struct Command {
    /// Ignore invalid packages
    #[arg(short, long)]
    ignore: bool,

    /// Target repo
    #[arg(short, long)]
    repo: Option<String>,

    // positionals
    /// Target packages or paths
    #[arg(
        value_name = "TARGET",
        // default to the current working directory
        default_value = ".",
        // default to all packages when targeting a repo
        default_value_if("repo", ArgPredicate::IsPresent, Some("*")),
        help_heading = "Arguments",
    )]
    targets: Vec<MaybeStdinVec<String>>,
}

// TODO: use configured ebuild repos
// TODO: support binpkg repos
/// Run pkg_pretend() phase for a package.
fn pretend(result: pkgcraft::Result<EbuildPkg>) -> pkgcraft::Result<Option<String>> {
    result.and_then(|pkg| pkg.pretend())
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // convert targets to pkgs
        let pkgs = Targets::new(config)?
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .pkg_targets(self.targets.iter().flatten())?
            .collapse()
            .ebuild_pkgs();

        // run pkg_pretend across selected pkgs
        let mut stdout = io::stdout().lock();
        let iter = pkgs.par_map_ordered(pretend).log_errors(self.ignore);
        let failed = iter.failed.clone();
        for output in iter.flatten() {
            writeln!(stdout, "{output}")?;
        }

        Ok(ExitCode::from(failed.get() as u8))
    }
}
