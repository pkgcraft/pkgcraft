use std::io::{self, Write};
use std::process::ExitCode;
use std::str::FromStr;
use std::time::{Duration, Instant};

use clap::Args;
use pkgcraft::cli::{MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcraft::pkg::{ebuild::EbuildRawPkg, Source};
use pkgcraft::repo::RepoFormat;
use pkgcraft::utils::bounded_jobs;
use pkgcraft::Error;
use scallop::pool::PoolIter;
use tracing::error;

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

        let val: humantime::Duration = val.parse()?;
        Ok(bound(val.into()))
    }
}

#[derive(Debug, Args)]
pub(crate) struct Command {
    /// Parallel jobs to run (default: # of physical CPUs)
    #[arg(short, long)]
    jobs: Option<usize>,

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
    /// Target packages or paths
    #[arg(value_name = "TARGET", default_value = ".")]
    targets: Vec<MaybeStdinVec<String>>,
}

// Truncate a duration to microsecond precision.
macro_rules! micros {
    ($val:expr) => {{
        let val = $val.as_micros().try_into().expect("duration overflow");
        Duration::from_micros(val)
    }};
}

/// Run package sourcing benchmarks for a given duration per package.
fn benchmark<I>(duration: Duration, jobs: usize, pkgs: I, sort: bool) -> anyhow::Result<bool>
where
    I: Iterator<Item = EbuildRawPkg>,
{
    let mut failed = false;
    let func = |pkg: EbuildRawPkg| -> scallop::Result<(String, Vec<Duration>)> {
        let mut data = vec![];
        let mut elapsed = Duration::new(0, 0);
        while elapsed < duration {
            let start = Instant::now();
            // TODO: move error mapping into pkgcraft for pkg sourcing
            pkg.source().map_err(|e| Error::InvalidPkg {
                id: pkg.to_string(),
                err: e.to_string(),
            })?;
            let source_elapsed = micros!(start.elapsed());
            data.push(source_elapsed);
            elapsed += source_elapsed;
            scallop::shell::reset(&[]);
        }
        Ok((pkg.to_string(), data))
    };

    let mut sorted = if sort { Some(vec![]) } else { None };
    let mut stdout = io::stdout().lock();

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
                    writeln!(
                        stdout,
                        "{pkg}: mean: {mean:?}, min: {min:?}, max: {max:?}, σ = {sdev:?}, N = {n}"
                    )?;
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
            writeln!(
                stdout,
                "{pkg}: mean: {mean:?}, min: {min:?}, max: {max:?}, σ = {sdev:?}, N = {n}"
            )?;
        }
    }

    Ok(failed)
}

/// Run package sourcing a single time per package.
fn source<I>(jobs: usize, pkgs: I, bound: &[Bound], sort: bool) -> anyhow::Result<bool>
where
    I: Iterator<Item = EbuildRawPkg>,
{
    let mut failed = false;
    let func = |pkg: EbuildRawPkg| -> scallop::Result<(String, Duration)> {
        let start = Instant::now();
        // TODO: move error mapping into pkgcraft for pkg sourcing
        pkg.source().map_err(|e| Error::InvalidPkg {
            id: pkg.to_string(),
            err: e.to_string(),
        })?;
        let elapsed = micros!(start.elapsed());
        Ok((pkg.to_string(), elapsed))
    };

    let mut sorted = if sort { Some(vec![]) } else { None };
    let mut stdout = io::stdout().lock();

    for r in PoolIter::new(jobs, pkgs, func, true)? {
        match r {
            Ok((pkg, elapsed)) => {
                if bound.iter().all(|b| b.matches(&elapsed)) {
                    if let Some(values) = sorted.as_mut() {
                        values.push((pkg, elapsed));
                    } else {
                        writeln!(stdout, "{pkg}: {elapsed:?}")?;
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
            writeln!(stdout, "{pkg}: {elapsed:?}")?;
        }
    }

    Ok(failed)
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // default to running a job on each physical CPU in order to limit contention
        let jobs = bounded_jobs(self.jobs.unwrap_or(num_cpus::get_physical()));

        // loop over targets, tracking overall failure status
        let mut status = ExitCode::SUCCESS;

        // find matching packages
        let (_pool, pkgs) = TargetRestrictions::new(config)
            .repo_format(RepoFormat::Ebuild)
            .pkgs_ebuild_raw(self.targets.iter().flatten())?;

        let target_failed = if let Some(duration) = self.bench {
            benchmark(duration.into(), jobs, pkgs, self.sort)
        } else {
            source(jobs, pkgs, &self.bound, self.sort)
        }?;

        if target_failed {
            status = ExitCode::FAILURE;
        }

        Ok(status)
    }
}
