use std::io::stdin;
use std::process::ExitCode;
use std::str::FromStr;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::pkg::ebuild::RawPkg;
use pkgcraft::pkg::SourceablePackage;
use pkgcraft::repo::Repo;
use pkgcraft::restrict::{self, Restrict};
use scallop::pool::PoolIter;
use tracing::error;

use crate::StdinArgs;

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
    #[arg(short, long, default_value = "gentoo", required = false)]
    repo: String,

    /// Benchmark sourcing for a given number of seconds per package
    #[arg(long, required = false)]
    bench: Option<u64>,

    /// Elapsed time bound to apply
    #[arg(short, long, required = false)]
    bound: Option<Bound>,

    // positionals
    /// Target packages
    #[arg(value_name = "PKG", required = false)]
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

        for r in PoolIter::new(jobs, pkgs, func)? {
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

        for r in PoolIter::new(jobs, pkgs, func)? {
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
        let mut restricts = vec![];

        if self.vals.stdin_args()? {
            for line in stdin().lines() {
                for s in line?.split_whitespace() {
                    restricts.push(restrict::parse::dep(s)?);
                }
            }
        } else {
            for s in &self.vals {
                restricts.push(restrict::parse::dep(s)?);
            }
        }

        // combine restricts into a single entity
        let restrict = Restrict::and(restricts);

        // determine target repo
        let repo = match config.repos.get(&self.repo) {
            Some(r) => Ok(r.clone()),
            None => Repo::from_path(&self.repo, 0, &self.repo, true),
        };

        let repo = repo.map_err(|_| anyhow!("unknown repo: {}", self.repo))?;
        let repo = repo
            .as_ebuild()
            .ok_or_else(|| anyhow!("non-ebuild repo: {repo}"))?;

        let jobs = self.jobs.unwrap_or_else(num_cpus::get_physical);
        let pkgs = repo.iter_raw_restrict(restrict);

        let failed = if let Some(secs) = self.bench {
            self.benchmark(secs, jobs, pkgs)
        } else {
            self.source(jobs, pkgs)
        }?;

        if failed {
            Ok(ExitCode::FAILURE)
        } else {
            Ok(ExitCode::SUCCESS)
        }
    }
}
