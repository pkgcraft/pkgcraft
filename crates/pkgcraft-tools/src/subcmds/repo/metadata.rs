use std::process::ExitCode;

use camino::Utf8Path;
use pkgcraft::config::Config;
use pkgcraft::repo::EbuildRepo;
use pkgcraft::repo::ebuild::{Cache, CacheFormat, MetadataCache};

mod clean;
mod regen;
mod remove;

#[derive(clap::Args)]
pub(crate) struct Command {
    #[command(subcommand)]
    command: Subcommand,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        self.command.run(config)
    }
}

#[derive(clap::Subcommand)]
enum Subcommand {
    /// Clean metadata cache
    Clean(clean::Command),
    /// Regenerate metadata cache
    Regen(regen::Command),
    /// Remove metadata cache
    Remove(remove::Command),
}

impl Subcommand {
    fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        match self {
            Self::Clean(cmd) => cmd.run(config),
            Self::Regen(cmd) => cmd.run(config),
            Self::Remove(cmd) => cmd.run(config),
        }
    }
}

/// Return the requested metadata caches for a given repo.
fn repo_caches<'a>(
    repo: &'a EbuildRepo,
    formats: &'a [CacheFormat],
    path: Option<&'a Utf8Path>,
) -> anyhow::Result<Box<dyn Iterator<Item = MetadataCache> + 'a>> {
    match (formats, path) {
        ([], None) => Ok(Box::new(repo.metadata().caches().values().cloned())),
        ([], Some(path)) => {
            let format = repo.metadata().cache().format();
            let cache = format.from_path(path);
            Ok(Box::new([cache].into_iter()))
        }
        ([format], None) => Ok(Box::new([format.from_repo(repo)].into_iter())),
        ([format], Some(path)) => Ok(Box::new([format.from_path(path)].into_iter())),
        (_, Some(_)) => anyhow::bail!("-p/--path incompatible with multiple caches"),
        (_, None) => Ok(Box::new(formats.iter().map(move |f| f.from_repo(repo)))),
    }
}
