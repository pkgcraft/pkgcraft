use std::collections::HashSet;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::{builder::ArgPredicate, Args};
use pkgcraft::cli::{MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcraft::pkg::ebuild::metadata::Key;
use pkgcraft::pkg::{ebuild::EbuildRawPkg, Source};
use pkgcraft::repo::RepoFormat;
use pkgcraft::shell::environment::Variable;
use pkgcraft::utils::bounded_jobs;
use scallop::pool::PoolIter;
use scallop::variables::{self, ShellVariable};
use strum::IntoEnumIterator;

#[derive(Args)]
#[clap(next_help_heading = "Env options")]
pub(crate) struct Command {
    /// Parallel jobs to run
    #[arg(short, long, default_value_t = num_cpus::get())]
    jobs: usize,

    /// Variable filtering
    #[arg(short, long)]
    filter: Vec<String>,

    /// Target repo
    #[arg(short, long)]
    repo: Option<String>,

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

// TODO: support other repo types such as configured and binpkg
impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let external: HashSet<_> = variables::visible().into_iter().collect();
        let bash: HashSet<_> = ["PIPESTATUS"].into_iter().collect();
        let pms: HashSet<_> = Variable::iter().map(|v| v.to_string()).collect();
        let meta: HashSet<_> = Key::iter().map(|v| v.to_string()).collect();

        // create variable filters
        let (mut hide, mut show) = (HashSet::new(), HashSet::new());
        let items = self.filter.iter().flat_map(|line| line.split(','));
        for item in items {
            // determine filter set
            let (set, var) = match item.strip_prefix('-') {
                Some(var) => (&mut hide, var),
                None => (&mut show, item),
            };

            // expand variable aliases
            match var {
                "@PMS" => set.extend(pms.iter().map(|s| s.as_str())),
                "@META" => set.extend(meta.iter().map(|s| s.as_str())),
                _ => {
                    set.insert(var);
                }
            }
        }

        let filter = |var: &variables::Variable| -> bool {
            let name = var.name();
            !external.contains(name)
                && !bash.contains(name)
                && !hide.contains(name)
                && (show.is_empty() || show.contains(name))
        };

        let value = |var: variables::Variable| -> Option<(String, String)> {
            var.to_vec().map(|v| (var.to_string(), v.join(" ")))
        };

        let func = |pkg: pkgcraft::Result<EbuildRawPkg>| -> scallop::Result<(String, Vec<(String, String)>)> {
            let pkg = pkg?;
            // TODO: move error mapping into pkgcraft for pkg sourcing
            pkg.source().map_err(|e| {
                let err: pkgcraft::Error = e.into();
                err.into_invalid_pkg_err(&pkg)
            })?;

            let env: Vec<(_, _)> = variables::visible()
                .into_iter()
                .filter(filter)
                .filter_map(value)
                .collect();

            Ok((pkg.to_string(), env))
        };

        // loop over targets, tracking overall failure status
        let jobs = bounded_jobs(self.jobs);
        let mut failed = false;

        // convert targets to pkgs
        let pkgs = TargetRestrictions::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .finalize_targets(self.targets.iter().flatten())?
            .ebuild_raw_pkgs();

        // source ebuilds and output ebuild-specific environment variables
        let (mut stdout, mut stderr) = (io::stdout().lock(), io::stderr().lock());
        let mut iter = PoolIter::new(jobs, pkgs, func, true)?.peekable();
        let mut multiple = false;
        while let Some(result) = iter.next() {
            match result {
                Err(e) => {
                    failed = true;
                    writeln!(stderr, "{e}")?;
                }
                Ok((_, env)) if env.is_empty() => continue,
                Ok((pkg, env)) => {
                    // determine if the header and footer should be displayed
                    let (header, footer) = match iter.peek() {
                        Some(Ok(_)) => {
                            multiple = true;
                            (multiple, true)
                        }
                        None => (multiple, false),
                        _ => (multiple, true),
                    };

                    if header {
                        writeln!(stdout, "{pkg}")?;
                    }
                    for (k, v) in env {
                        writeln!(stdout, "{k}={v}")?;
                    }
                    if footer {
                        writeln!(stdout)?;
                    }
                }
            }
        }

        Ok(ExitCode::from(failed as u8))
    }
}
