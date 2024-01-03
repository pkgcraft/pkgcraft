use std::io::{stdin, IsTerminal};

use camino::Utf8Path;
use pkgcraft::config::Config;
use pkgcraft::repo::set::RepoSet;
use pkgcraft::restrict::{self, Restrict};

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

/// Convert a target into a path or dep restriction.
pub(crate) fn target_restriction(
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
