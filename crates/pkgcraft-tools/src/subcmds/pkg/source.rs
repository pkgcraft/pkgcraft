use std::io::{self, Write};
use std::num::NonZero;
use std::process::ExitCode;
use std::str::FromStr;
use std::time::{Duration, Instant};

use clap::{builder::ArgPredicate, Args};
use pkgcraft::cli::{MaybeStdinVec, PkgTargets, Targets};
use pkgcraft::config::Config;
use pkgcraft::pkg::ebuild::EbuildRawPkg;
use pkgcraft::repo::RepoFormat;
use pkgcraft::traits::ParallelMapIter;
use pkgcraft::utils::bounded_thread_pool;
use tracing::error;

/// Duration bound to apply against elapsed time values.
#[derive(Copy, Clone)]
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
            if let Some(v) = s.strip_prefix('>') {
                if let Some(v) = v.strip_prefix('=') {
                    (Self::GreaterOrEqual, v)
                } else {
                    (Self::Greater, v)
                }
            } else if let Some(v) = s.strip_prefix('<') {
                if let Some(v) = v.strip_prefix('=') {
                    (Self::LessOrEqual, v)
                } else {
                    (Self::Less, v)
                }
            } else {
                (Self::GreaterOrEqual, s)
            }
        };

        let val: humantime::Duration = val.parse()?;
        Ok(bound(val.into()))
    }
}

/// Perform a benchmark for an elapsed duration or number of runs.
#[derive(Copy, Clone)]
enum Bench {
    Duration(Duration),
    Runs(NonZero<u32>),
}

impl FromStr for Bench {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        if let Ok(value) = humantime::Duration::from_str(s) {
            Ok(Self::Duration(value.into()))
        } else if let Ok(value) = s.parse() {
            Ok(Self::Runs(value))
        } else {
            Err(anyhow::anyhow!("invalid benchmark value: {s}"))
        }
    }
}

#[derive(Args)]
#[clap(next_help_heading = "Source options")]
pub(crate) struct Command {
    /// Parallel jobs to run
    #[arg(short, long, default_value_t = num_cpus::get_physical())]
    jobs: usize,

    /// Benchmark for a duration or number of runs
    #[arg(short, long)]
    bench: Option<Bench>,

    /// Apply bounds to elapsed time
    #[arg(short = 'B', long)]
    bound: Vec<Bound>,

    /// Benchmark across all targets cumulatively
    #[arg(
        short,
        long,
        value_name = "RUNS",
        num_args = 0..=1,
        default_missing_value = "1",
        conflicts_with = "bench"
    )]
    cumulative: Option<NonZero<u32>>,

    /// Target repo
    #[arg(short, long)]
    repo: Option<String>,

    /// Sort output in ascending order for elapsed time
    #[arg(long, conflicts_with = "cumulative")]
    sort: bool,

    // positionals
    /// Target packages or paths
    #[arg(
        value_name = "TARGET",
        // default to the current working directory
        default_value = ".",
        // default to all packages when targeting a repo
        default_value_if("repo", ArgPredicate::IsPresent, Some("*")),
        help_heading = "Arguments",
    )]
    targets: Vec<MaybeStdinVec<String>>,
}

// Truncate a duration to millisecond precision.
macro_rules! millis {
    ($val:expr) => {{
        let val = $val.as_millis().try_into().expect("duration overflow");
        Duration::from_millis(val)
    }};
}

// Truncate a duration to microsecond precision.
macro_rules! micros {
    ($val:expr) => {{
        let val = $val.as_micros().try_into().expect("duration overflow");
        Duration::from_micros(val)
    }};
}

/// Run package sourcing benchmarks for a given duration per package.
fn benchmark(bench: Bench, targets: PkgTargets, sort: bool) -> anyhow::Result<bool> {
    let mut failed = false;
    let func =
        move |pkg: pkgcraft::Result<EbuildRawPkg>| -> scallop::Result<(String, Vec<Duration>)> {
            let pkg = pkg?;
            let mut data = vec![];
            match bench {
                Bench::Duration(duration) => {
                    let start = Instant::now();
                    while start.elapsed() < duration {
                        let time = pkg.duration()?;
                        data.push(micros!(time));
                    }
                }
                Bench::Runs(limit) => {
                    let mut runs = 0;
                    while runs < limit.get() {
                        let time = pkg.duration()?;
                        data.push(micros!(time));
                        runs += 1;
                    }
                }
            }
            Ok((pkg.to_string(), data))
        };

    let pkgs = targets.ebuild_raw_pkgs();
    let mut sorted = if sort { Some(vec![]) } else { None };
    let mut stdout = io::stdout().lock();

    for result in ParallelMapIter::new(pkgs, func) {
        match result {
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

/// Run package sourcing benchmark cumulatively across all targets.
fn cumulative(limit: u32, targets: PkgTargets) -> anyhow::Result<bool> {
    let func = move |pkg: pkgcraft::Result<EbuildRawPkg>| -> scallop::Result<Duration> {
        Ok(pkg?.duration()?)
    };

    let mut failed = false;
    let mut run = 0;
    let mut values = vec![];
    let mut stdout = io::stdout().lock();

    while run < limit {
        let mut cpu_time = Duration::new(0, 0);
        let start = Instant::now();
        let pkgs = targets.clone().ebuild_raw_pkgs();

        for result in ParallelMapIter::new(pkgs, func) {
            match result {
                Ok(duration) => cpu_time += duration,
                Err(e) => {
                    failed = true;
                    error!("{e}");
                }
            }
        }

        run += 1;
        let elapsed = millis!(start.elapsed());
        let cpu_time = millis!(cpu_time);
        writeln!(stdout, "run #{run}: real: {elapsed:?}, cpu: {cpu_time:?}")?;
        values.push(elapsed);
    }

    // output statistics across multiple runs
    if limit > 1 {
        let n = run as u64;
        let millis: Vec<u64> = values
            .iter()
            .map(|v| v.as_millis().try_into().unwrap())
            .collect();
        let min = Duration::from_millis(*millis.iter().min().unwrap());
        let max = Duration::from_millis(*millis.iter().max().unwrap());
        let total: u64 = millis.iter().sum();
        let mean: u64 = total / n;
        let variance = (millis
            .iter()
            .map(|v| (*v as i64 - mean as i64).pow(2))
            .sum::<i64>()) as f64
            / n as f64;
        let sdev = Duration::from_millis(variance.sqrt().round() as u64);
        let mean = Duration::from_millis(mean);

        writeln!(
            stdout,
            "total: mean: {mean:?}, min: {min:?}, max: {max:?}, σ = {sdev:?}, N = {n}"
        )?;
    }

    Ok(failed)
}

/// Run package sourcing a single time per package.
fn source(targets: PkgTargets, bound: &[Bound], sort: bool) -> anyhow::Result<bool> {
    let mut failed = false;
    let func =
        move |pkg: pkgcraft::Result<EbuildRawPkg>| -> scallop::Result<(String, Duration)> {
            let pkg = pkg?;
            let elapsed = micros!(pkg.duration()?);
            Ok((pkg.to_string(), elapsed))
        };

    let pkgs = targets.ebuild_raw_pkgs();
    let mut sorted = if sort { Some(vec![]) } else { None };
    let mut stdout = io::stdout().lock();

    for result in ParallelMapIter::new(pkgs, func) {
        match result {
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
        // build custom, global thread pool when limiting jobs
        bounded_thread_pool(self.jobs);

        // convert targets to pkgs
        let targets = Targets::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .finalize_pkgs(self.targets.iter().flatten())?;

        let failed = if let Some(value) = self.bench {
            benchmark(value, targets, self.sort)
        } else if let Some(value) = self.cumulative {
            cumulative(value.get(), targets)
        } else {
            source(targets, &self.bound, self.sort)
        }?;

        Ok(ExitCode::from(failed as u8))
    }
}
