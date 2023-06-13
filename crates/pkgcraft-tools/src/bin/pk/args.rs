use std::io::stdin;

use is_terminal::IsTerminal;
use itertools::Either;

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
