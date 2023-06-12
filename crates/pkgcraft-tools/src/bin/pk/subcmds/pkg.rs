use std::path::Path;
use std::process::ExitCode;

use pkgcraft::config::Config;
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::RepoFormat::Ebuild as EbuildRepo;
use pkgcraft::restrict::{self, Restrict};

mod pretend;
mod source;

#[derive(Debug, clap::Args)]
pub struct Command {
    #[command(subcommand)]
    command: Subcommand,
}

impl Command {
    pub(super) fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        self.command.run(config)
    }
}

fn target_restriction(reposet: &RepoSet, target: &str) -> anyhow::Result<(RepoSet, Restrict)> {
    let path = Path::new(target);

    if path.exists() {
        let mut ebuild_repos = reposet.repos().iter().filter_map(|r| r.as_ebuild());
        if let Some(r) = ebuild_repos.find_map(|r| r.restrict_from_path(target)) {
            // target is an configured repo path restrict
            return Ok((reposet.clone(), r));
        } else if let Ok(repo) = EbuildRepo.load_from_nested_path(target, 0, target, true) {
            // target is an external repo path restrict
            let restrict = repo
                .as_ebuild()
                .unwrap()
                .restrict_from_path(target)
                .unwrap();
            return Ok((RepoSet::new([&repo]), restrict));
        }
    }

    if let Ok(r) = restrict::parse::dep(target) {
        // target is a regular dep restrict
        return Ok((reposet.clone(), r));
    }

    anyhow::bail!("invalid target: {target}")
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Run the pkg_pretend phase
    Pretend(pretend::Command),
    /// Source ebuilds and dump elapsed time
    Source(source::Command),
}

impl Subcommand {
    fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Pretend(cmd) => cmd.run(config),
            Source(cmd) => cmd.run(config),
        }
    }
}
