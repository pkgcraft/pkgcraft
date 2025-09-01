use std::collections::HashSet;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::{Args, builder::ArgPredicate};
use globset::{Glob, GlobSetBuilder};
use indexmap::IndexMap;
use pkgcraft::cli::{MaybeStdinVec, Targets};
use pkgcraft::config::Config;
use pkgcraft::pkg::ebuild::EbuildRawPkg;
use pkgcraft::pkg::ebuild::metadata::Key;
use pkgcraft::repo::RepoFormat;
use pkgcraft::shell::environment::Variable;
use pkgcraft::traits::LogErrors;
use strum::IntoEnumIterator;

#[derive(Args)]
#[clap(next_help_heading = "Env options")]
pub(crate) struct Command {
    /// Variable filtering
    #[arg(
        short,
        long,
        value_name = "FILTER[,....]",
        value_delimiter = ',',
        allow_hyphen_values = true
    )]
    filter: Vec<String>,

    /// Ignore invalid packages
    #[arg(short, long)]
    ignore: bool,

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

/// Get the environment data from a raw ebuild package.
fn get_env(
    result: pkgcraft::Result<EbuildRawPkg>,
) -> pkgcraft::Result<(String, IndexMap<String, String>)> {
    result.and_then(|pkg| pkg.env().map(|env| (pkg.to_string(), env)))
}

// TODO: support other repo types such as configured and binpkg
impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // convert targets to pkgs
        let pkgs = Targets::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .pkg_targets(self.targets.iter().flatten())?
            .collapse()
            .ebuild_raw_pkgs();

        let eapi: HashSet<_> = Variable::iter().map(|v| v.to_string()).collect();
        let meta: HashSet<_> = Key::iter().map(|v| v.to_string()).collect();

        // create variable filters
        let (mut hide, mut show) = (GlobSetBuilder::new(), GlobSetBuilder::new());
        let items = self.filter.iter().flat_map(|line| line.split(','));
        for item in items {
            // determine filter set
            let (set, var) = match item.strip_prefix('-') {
                Some(var) => (&mut hide, var),
                None => (&mut show, item),
            };

            // expand variable aliases
            match var {
                "@EAPI" => {
                    for s in eapi.iter() {
                        set.add(Glob::new(s)?);
                    }
                }
                "@META" => {
                    for s in meta.iter() {
                        set.add(Glob::new(s)?);
                    }
                }
                _ => {
                    set.add(Glob::new(var)?);
                }
            }
        }

        let hide = hide.build()?;
        let show = show.build()?;

        // filter variables being shown
        let filter = |name: &str| -> bool {
            !hide.is_match(name) && (show.is_empty() || show.is_match(name))
        };

        // source ebuilds and output ebuild-specific environment variables
        let mut stdout = io::stdout().lock();
        let iter = pkgs.into_iter().map(get_env).log_errors(self.ignore);
        let failed = iter.failed.clone();
        let mut iter = iter.peekable();
        let mut multiple = false;
        while let Some((pkg, env)) = iter.next() {
            // determine if the header and footer should be displayed
            let (header, footer) = match iter.peek() {
                Some(_) => {
                    multiple = true;
                    (multiple, true)
                }
                None => (multiple, false),
            };

            if header {
                writeln!(stdout, "{pkg}")?;
            }
            for (name, value) in env.iter().filter(|(name, _)| filter(name)) {
                writeln!(stdout, "{name}={value}")?;
            }
            if footer {
                writeln!(stdout)?;
            }
        }

        Ok(ExitCode::from(failed.get() as u8))
    }
}
