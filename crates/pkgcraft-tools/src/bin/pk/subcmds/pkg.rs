use std::process::ExitCode;

use camino::Utf8Path;
use pkgcraft::config::Config;
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::Repository;
use pkgcraft::restrict::{self, Restrict};

mod env;
mod metadata;
mod pretend;
mod revdeps;
mod source;

#[derive(Debug, clap::Args)]
pub struct Command {
    #[command(subcommand)]
    command: Subcommand,
}

impl Command {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        self.command.run(config)
    }
}

/// Convert a target into a path or dep restriction.
fn target_restriction(
    config: &mut Config,
    repos: &RepoSet,
    target: &str,
    cpv: bool,
) -> anyhow::Result<(RepoSet, Restrict)> {
    let path_target = Utf8Path::new(target).canonicalize_utf8();

    if let Ok(path) = &path_target {
        if path.exists() {
            if let Some(r) = repos.ebuild().find_map(|r| r.restrict_from_path(path, cpv)) {
                // target is an configured repo path restrict
                return Ok((repos.clone(), r));
            } else {
                // target is an external repo path restrict
                let repo = config.add_nested_repo_path(path.as_str(), 0, path, true)?;
                if let Some(r) = repo.as_ebuild() {
                    let restrict = r.restrict_from_path(path, cpv).unwrap();
                    return Ok((RepoSet::from_iter([&repo]), restrict));
                } else {
                    anyhow::bail!("non-ebuild repo: {repo}")
                }
            }
        }
    }

    match (restrict::parse::dep(target), path_target) {
        (Ok(restrict), _) => Ok((repos.clone(), restrict)),
        (_, Ok(path)) if path.exists() => anyhow::bail!("invalid repo path: {path}"),
        (_, Err(e)) if target.starts_with(['.', '/']) => {
            anyhow::bail!("invalid path target: {target}: {e}")
        }
        (Err(e), _) => anyhow::bail!(e),
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Output global environment
    Env(env::Command),
    /// Generate package metadata
    Metadata(metadata::Command),
    /// Run the pkg_pretend phase
    Pretend(pretend::Command),
    /// Output reverse dependencies
    Revdeps(revdeps::Command),
    /// Source ebuilds and dump elapsed time
    Source(source::Command),
}

impl Subcommand {
    fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Env(cmd) => cmd.run(config),
            Metadata(cmd) => cmd.run(config),
            Pretend(cmd) => cmd.run(config),
            Revdeps(cmd) => cmd.run(config),
            Source(cmd) => cmd.run(config),
        }
    }
}
