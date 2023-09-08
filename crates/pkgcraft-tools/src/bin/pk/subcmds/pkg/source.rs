use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr;
use std::time::{Duration, Instant};

use clap::Args;
use pkgcraft::config::{Config, Repos};
use pkgcraft::pkg::ebuild::RawPkg;
use pkgcraft::pkg::SourceablePackage;
use pkgcraft::repo::set::RepoSet;
use pkgcraft::utils::bounded_jobs;
use scallop::pool::PoolIter;
use tracing::error;

use crate::args::StdinOrArgs;

use super::target_restriction;

/// Duration bound to apply against elapsed time values.
#[derive(Debug, Copy, Clone)]
enum Bound {
    Less(Duration),
    LessOrEqual(Duration),
    Greater(Duration),
    GreaterOrEqual(Duration),
}

impl Bound {
    fn matches(&self, duration: &Duration) -> bool {
        match self {
            Self::Less(bound) => duration < bound,
            Self::LessOrEqual(bound) => duration <= bound,
            Self::GreaterOrEqual(bound) => duration >= bound,
            Self::Greater(bound) => duration > bound,
        }
    }
}

impl FromStr for Bound {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let (bound, val): (fn(Duration) -> Self, &str) = {
            // TODO: use an actual parser
            if let Some(v) = s.strip_prefix(">=") {
                (Self::GreaterOrEqual, v)
            } else if let Some(v) = s.strip_prefix('>') {
                (Self::Greater, v)
            } else if let Some(v) = s.strip_prefix("<=") {
                (Self::LessOrEqual, v)
            } else if let Some(v) = s.strip_prefix('<') {
                (Self::Less, v)
            } else {
                (Self::GreaterOrEqual, s)
            }
        };

        let val = humantime::Duration::from_str(val)?;
        Ok(bound(val.into()))
    }
}

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run (default: # of physical CPUs)
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Target repository
    #[arg(short, long)]
    repo: Option<String>,

    /// Benchmark sourcing for a given duration per package
    #[arg(long)]
    bench: Option<humantime::Duration>,

    /// Bounds applied to elapsed time
    #[arg(short, long)]
    bound: Vec<Bound>,

    /// Sort output in ascending order for elapsed time
    #[arg(long)]
    sort: bool,

    // positionals
    /// Target packages or directories
    #[arg(value_name = "TARGET", default_value = ".")]
    targets: Vec<String>,
}

// Truncate a duration to microsecond precision.
macro_rules! micros {
    ($val:expr) => {{
        let val = $val.as_micros().try_into().expect("duration overflow");
        Duration::from_micros(val)
    }};
}

/// Run package sourcing benchmarks for a given duration per package.
fn benchmark<'a, I>(duration: Duration, jobs: usize, pkgs: I, sort: bool) -> anyhow::Result<bool>
where
    I: Iterator<Item = RawPkg<'a>>,
{
    let mut failed = false;
    let func = |pkg: RawPkg| -> scallop::Result<(String, Vec<Duration>)> {
        let mut data = vec![];
        let mut elapsed = Duration::new(0, 0);
        while elapsed < duration {
            let start = Instant::now();
            pkg.source()?;
            let source_elapsed = micros!(start.elapsed());
            data.push(source_elapsed);
            elapsed += source_elapsed;
            scallop::shell::reset(&[]);
        }
        Ok((pkg.to_string(), data))
    };

    let mut sorted = if sort { Some(vec![]) } else { None };

    for r in PoolIter::new(jobs, pkgs, func, true)? {
        match r {
            Ok((pkg, data)) => {
                let n = data.len() as u64;
                let micros: Vec<u64> = data
                    .iter()
                    .map(|v| v.as_micros().try_into().unwrap())
                    .collect();
                let min = Duration::from_micros(*micros.iter().min().unwrap());
                let max = Duration::from_micros(*micros.iter().max().unwrap());
                let total: u64 = micros.iter().sum();
                let mean: u64 = total / n;
                let variance = (micros
                    .iter()
                    .map(|v| (*v as i64 - mean as i64).pow(2))
                    .sum::<i64>()) as f64
                    / n as f64;
                let sdev = Duration::from_micros(variance.sqrt().round() as u64);
                let mean = Duration::from_micros(mean);
                if let Some(values) = sorted.as_mut() {
                    values.push((pkg, mean, min, max, sdev, n));
                } else {
                    println!(
                        "{pkg}: mean: {mean:?}, min: {min:?}, max: {max:?}, σ = {sdev:?}, N = {n}"
                    );
                }
            }
            Err(e) => {
                failed = true;
                error!("{e}");
            }
        }
    }

    // output in ascending order if sorting is enabled
    if let Some(values) = sorted.as_mut() {
        values.sort_by(|(_, t1, ..), (_, t2, ..)| t1.cmp(t2));
        for (pkg, mean, min, max, sdev, n) in values {
            println!("{pkg}: mean: {mean:?}, min: {min:?}, max: {max:?}, σ = {sdev:?}, N = {n}");
        }
    }

    Ok(failed)
}

/// Run package sourcing a single time per package.
fn source<'a, I>(jobs: usize, pkgs: I, bound: &[Bound], sort: bool) -> anyhow::Result<bool>
where
    I: Iterator<Item = RawPkg<'a>>,
{
    let mut failed = false;
    let func = |pkg: RawPkg| -> scallop::Result<(String, Duration)> {
        let start = Instant::now();
        pkg.source()?;
        let elapsed = micros!(start.elapsed());
        Ok((pkg.to_string(), elapsed))
    };

    let mut sorted = if sort { Some(vec![]) } else { None };
    let mut handle = io::stdout().lock();

    for r in PoolIter::new(jobs, pkgs, func, true)? {
        match r {
            Ok((pkg, elapsed)) => {
                if bound.iter().all(|b| b.matches(&elapsed)) {
                    if let Some(values) = sorted.as_mut() {
                        values.push((pkg, elapsed));
                    } else {
                        writeln!(handle, "{pkg}: {elapsed:?}")?;
                    }
                }
            }
            Err(e) => {
                failed = true;
                error!("{e}");
            }
        }
    }

    // output in ascending order if sorting is enabled
    if let Some(values) = sorted.as_mut() {
        values.sort_by(|(_, t1), (_, t2)| t1.cmp(t2));
        for (pkg, elapsed) in values {
            writeln!(handle, "{pkg}: {elapsed:?}")?;
        }
    }

    Ok(failed)
}

impl Command {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine target repo set
        let repos = if let Some(repo) = self.repo.as_ref() {
            let repo = if let Some(r) = config.repos.get(repo) {
                Ok(r.clone())
            } else if Path::new(repo).exists() {
                config.add_nested_repo_path(repo, 0, repo, true)
            } else {
                anyhow::bail!("unknown repo: {repo}")
            }?;
            RepoSet::new([&repo])
        } else {
            config.repos.set(Repos::Ebuild)
        };

        // default to running a job on each physical CPU in order to limit contention
        let jobs = bounded_jobs(self.jobs.or(Some(num_cpus::get_physical())));

        // loop over targets, tracking overall failure status
        let mut status = ExitCode::SUCCESS;
        for target in self.targets.stdin_or_args().split_whitespace() {
            // determine target restriction
            let (repos, restrict) = target_restriction(config, &repos, &target)?;

            // find matching packages from targeted repos
            let pkgs = repos.ebuild().flat_map(|r| r.iter_raw_restrict(&restrict));

            let target_failed = if let Some(duration) = self.bench {
                benchmark(duration.into(), jobs, pkgs, self.sort)
            } else {
                source(jobs, pkgs, &self.bound, self.sort)
            }?;

            if target_failed {
                status = ExitCode::FAILURE;
            }
        }

        Ok(status)
    }
}
