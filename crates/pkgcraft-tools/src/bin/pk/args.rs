use std::io::{stdin, IsTerminal};
use std::path::Path;
use std::sync::Arc;

use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::Repo as EbuildRepo;

pub(super) struct StdinArgs<'a> {
    iter: Box<dyn Iterator<Item = String> + 'a>,
}

impl<'a> StdinArgs<'a> {
    /// Split arguments into separate strings by whitespace.
    pub(super) fn split_whitespace(self) -> StdinArgs<'a> {
        let iter = self.iter.flat_map(|s| {
            s.split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        });

        StdinArgs { iter: Box::new(iter) }
    }
}

impl Iterator for StdinArgs<'_> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Pull values from stdin if it's not a terminal and the first argument is `-`, otherwise use the
/// provided arguments.
pub(super) trait StdinOrArgs {
    fn stdin_or_args<'a>(self) -> StdinArgs<'a>
    where
        Self: IntoIterator<Item = String> + Sized + 'a,
    {
        let mut iter = self.into_iter().peekable();
        let iter: Box<dyn Iterator<Item = String>> = match iter.peek().map(|s| s.as_str()) {
            Some("-") if !stdin().is_terminal() => Box::new(stdin().lines().map_while(Result::ok)),
            _ => Box::new(iter),
        };

        StdinArgs { iter }
    }
}

impl StdinOrArgs for Vec<String> {}

/// Convert a target ebuild repo arg into an ebuild repo.
pub(crate) fn target_ebuild_repo(
    config: &mut Config,
    repo: &str,
) -> anyhow::Result<Arc<EbuildRepo>> {
    let repo = if let Some(r) = config.repos.get(repo) {
        Ok(r.clone())
    } else if Path::new(repo).exists() {
        config.add_repo_path(repo, 0, repo, true)
    } else {
        anyhow::bail!("unknown repo: {repo}")
    }?;

    if let Some(r) = repo.as_ebuild() {
        Ok(r.clone())
    } else {
        anyhow::bail!("non-ebuild repo: {repo}")
    }
}

/// Convert a list of target ebuild repo args into ebuild repos.
pub(crate) fn target_ebuild_repos(
    config: &mut Config,
    args: &[String],
) -> anyhow::Result<Vec<Arc<EbuildRepo>>> {
    args.iter().map(|s| target_ebuild_repo(config, s)).collect()
}
