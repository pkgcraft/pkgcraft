use std::io::stdin;
use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr;
use std::time::{Duration, Instant};

use clap::Args;
use is_terminal::IsTerminal;
use itertools::Either;
use pkgcraft::config::{Config, RepoSetType};
use pkgcraft::pkg::ebuild::RawPkg;
use pkgcraft::pkg::SourceablePackage;
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::RepoFormat;
use pkgcraft::restrict;
use scallop::pool::PoolIter;
use tracing::error;

use crate::args::bounded_jobs;

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
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Target repository
    #[arg(short, long)]
    repo: Option<String>,

    /// Benchmark sourcing for a given number of seconds per package
    #[arg(long)]
    bench: Option<u64>,

    /// Elapsed time bound to apply
    #[arg(short, long)]
    bound: Option<Bound>,

    // positionals
    /// Target packages
    #[arg(value_name = "PKG")]
    vals: Vec<String>,
}

// Truncate a duration to microsecond precision.
macro_rules! micros {
    ($val:expr) => {{
        let val = $val.as_micros().try_into().expect("duration overflow");
        Duration::from_micros(val)
    }};
}

impl Command {
    /// Determine if a duration matches a given, optional bound.
    fn bounded(&self, elapsed: &Duration) -> bool {
        if let Some(bound) = self.bound {
            bound.matches(elapsed)
        } else {
            true
        }
    }

    /// Run package sourcing benchmarks for a given amount of seconds per package.
    fn benchmark<'a, I>(&self, secs: u64, jobs: usize, pkgs: I) -> anyhow::Result<bool>
    where
        I: Iterator<Item = RawPkg<'a>>,
    {
        let mut failed = false;
        let func = |pkg: RawPkg| -> scallop::Result<(String, Vec<Duration>)> {
            let mut data = vec![];
            let mut elapsed = Duration::new(0, 0);
            while elapsed.as_secs() < secs {
                let start = Instant::now();
                pkg.source()?;
                let source_elapsed = micros!(start.elapsed());
                data.push(source_elapsed);
                elapsed += source_elapsed;
                scallop::shell::reset(&[]);
            }
            Ok((pkg.to_string(), data))
        };

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
                    let variance =
                        (micros.iter().map(|v| (v - mean).pow(2)).sum::<u64>()) as f64 / n as f64;
                    let std_dev = Duration::from_micros(variance.sqrt().round() as u64);
                    let mean = Duration::from_micros(mean);
                    println!("{pkg}: min: {min:?}, mean: {mean:?}, max: {max:?}, σ = {std_dev:?}, N = {n}")
                }
                Err(e) => {
                    failed = true;
                    error!("{e}");
                }
            }
        }

        Ok(failed)
    }

    /// Run package sourcing a single time per package.
    fn source<'a, I>(&self, jobs: usize, pkgs: I) -> anyhow::Result<bool>
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

        for r in PoolIter::new(jobs, pkgs, func, true)? {
            match r {
                Ok((pkg, elapsed)) => {
                    if self.bounded(&elapsed) {
                        println!("{pkg}: {elapsed:?}")
                    }
                }
                Err(e) => {
                    failed = true;
                    error!("{e}");
                }
            }
        }

        Ok(failed)
    }

    pub(super) fn run(&self, config: &Config) -> anyhow::Result<ExitCode> {
        // determine target repo set
        let reposet = if let Some(repo) = self.repo.as_ref() {
            let repo = if let Some(r) = config.repos.get(repo) {
                Ok(r.clone())
            } else if Path::new(repo).exists() {
                RepoFormat::Ebuild.load_from_path(repo, 0, repo, true)
            } else {
                anyhow::bail!("unknown repo: {repo}")
            }?;
            RepoSet::new([&repo])
        } else {
            config.repos.set(RepoSetType::Ebuild)
        };

        // restrict searches to ebuild repos
        let repos: Vec<_> = reposet
            .repos()
            .iter()
            .filter_map(|r| r.as_ebuild())
            .collect();
        if repos.is_empty() {
            anyhow::bail!("no matching ebuild repos found");
        }

        // pull targets from args or stdin
        let args = if stdin().is_terminal() {
            Either::Left(self.vals.clone().into_iter())
        } else {
            Either::Right(stdin().lines().map_while(Result::ok))
        };

        // loop over targets, tracking overall failure status
        let jobs = bounded_jobs(self.jobs)?;
        let mut failed = false;
        for target in args {
            let restrict = restrict::parse::dep(&target)?;

            // convert repos into packages
            let pkgs = repos.iter().flat_map(|r| r.iter_raw_restrict(&restrict));

            let target_failed = if let Some(secs) = self.bench {
                self.benchmark(secs, jobs, pkgs)
            } else {
                self.source(jobs, pkgs)
            }?;

            if target_failed {
                failed = true;
            }
        }

        if failed {
            Ok(ExitCode::FAILURE)
        } else {
            Ok(ExitCode::SUCCESS)
        }
    }
}
