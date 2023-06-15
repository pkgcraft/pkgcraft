use std::io::stdin;
use std::path::Path;
use std::sync::Arc;

use is_terminal::IsTerminal;
use itertools::Either;
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::Repo as EbuildRepo;
use pkgcraft::repo::RepoFormat;

/// Limit parallel jobs to the number of logical CPUs on a system.
pub(crate) fn bounded_jobs(jobs: Option<usize>) -> anyhow::Result<usize> {
    let cpus = num_cpus::get();
    match jobs {
        Some(j) if j == 0 => anyhow::bail!("jobs must be a positive integer"),
        Some(j) if j <= cpus => Ok(j),
        _ => Ok(cpus),
    }
}

/// Pull values from stdin or arguments depending on if stdin is a terminal.
pub(crate) fn stdin_or_args<I>(args: I) -> impl Iterator<Item = String>
where
    I: IntoIterator<Item = String>,
{
    if stdin().is_terminal() {
        Either::Left(args.into_iter())
    } else {
        Either::Right(stdin().lines().map_while(Result::ok))
    }
}

/// Convert a target ebuild repo arg into an ebuild repo.
pub(crate) fn target_ebuild_repo(config: &Config, repo: &str) -> anyhow::Result<Arc<EbuildRepo>> {
    let repo = if let Some(r) = config.repos.get(repo) {
        Ok(r.clone())
    } else if Path::new(repo).exists() {
        RepoFormat::Ebuild.load_from_path(repo, 0, repo, true)
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
    config: &Config,
    args: &[String],
) -> anyhow::Result<Vec<Arc<EbuildRepo>>> {
    let mut repos = vec![];
    for target in args {
        repos.push(target_ebuild_repo(config, target)?);
    }
    Ok(repos)
}
