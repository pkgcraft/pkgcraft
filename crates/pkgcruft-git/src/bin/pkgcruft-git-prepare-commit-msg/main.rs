use std::process::ExitCode;
use std::{env, fs, io};

use anyhow::anyhow;
use camino::Utf8PathBuf;
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::cli::colorize;
use pkgcraft::repo::EbuildRepo;
use pkgcraft::utils::current_dir;
use tracing_log::AsTrace;

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    version,
    long_about = None,
    disable_help_subcommand = true,
)]
/// pkgcruft-git pre-commit hook
pub(crate) struct Command {
    #[command(flatten)]
    verbosity: Verbosity,

    #[command(flatten)]
    color: colorchoice_clap::Color,

    path: Utf8PathBuf,
    kind: Option<String>,
    commit: Option<String>,
}

fn generate_msg() -> anyhow::Result<Option<String>> {
    // load repo from the current working directory
    let path = current_dir()?;
    let repo = EbuildRepo::standalone(&path)?;
    let git_repo = git2::Repository::open(&repo)
        .map_err(|e| anyhow!("failed opening git repo: {path}: {e}"))?;

    // determine target Cpns from diff
    let tree = git_repo.head()?.peel_to_tree()?;
    let diff = git_repo.diff_tree_to_index(Some(&tree), None, None)?;
    let paths = diff.deltas().filter_map(|d| d.new_file().path());
    let cpns: IndexSet<_> = paths
        .filter_map(|p| repo.cpn_from_path(p).ok())
        .sorted()
        .collect();

    // TODO: support more types of messages, e.g. simple version bumps and removals
    let msg = if cpns.len() == 1
        && let Some(cpn) = cpns.first()
    {
        Some(cpn.to_string())
    } else {
        None
    };

    Ok(msg)
}

fn try_main() -> anyhow::Result<ExitCode> {
    let args = Command::parse();

    // set color choice
    args.color.write_global();

    // create formatting subscriber that uses stderr
    let level = args.verbosity.log_level_filter();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level.as_trace())
        .with_writer(io::stderr)
        .with_ansi(colorize!(&io::stderr()));

    // initialize global subscriber
    subscriber.init();

    // ignore commits with pre-existing message content
    if args.kind.is_none()
        && let Some(msg) = generate_msg()?
    {
        let data = fs::read_to_string(&args.path)?;
        fs::write(&args.path, format!("{msg}: \n{data}"))?;
    }

    Ok(ExitCode::SUCCESS)
}

fn main() -> anyhow::Result<ExitCode> {
    try_main().or_else(|e| {
        let cmd = env!("CARGO_BIN_NAME");
        eprintln!("{cmd}: error: {e}");
        Ok(ExitCode::from(1))
    })
}
