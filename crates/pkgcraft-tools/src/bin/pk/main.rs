use std::io::stderr;
use std::process::ExitCode;

use clap::Parser;
use clap_verbosity_flag::Verbosity;
use pkgcraft::cli::reset_sigpipe;
use pkgcraft::config::Config;
use tracing_log::AsTrace;

mod args;
mod format;
mod subcmds;

#[derive(Debug, Parser)]
#[command(version, long_about = None, disable_help_subcommand = true)]
/// pkgcraft command-line tool
struct Command {
    #[command(flatten)]
    verbosity: Verbosity,

    /// Use a custom config
    #[arg(short, long, value_name = "PATH", global = true)]
    config: Option<String>,

    // positional
    #[command(subcommand)]
    subcmd: subcmds::Subcommand,
}

fn main() -> anyhow::Result<ExitCode> {
    // reset SIGPIPE behavior since rust ignores it by default
    reset_sigpipe();

    let args = Command::parse();

    // custom log event formatter
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(false)
        .without_time()
        .compact();

    tracing_subscriber::fmt()
        .event_format(format)
        .with_max_level(args.verbosity.log_level_filter().as_trace())
        .with_writer(stderr)
        .init();

    let mut config = Config::new("pkgcraft", "");
    if let Some(path) = args.config {
        config.load_path(&path)?;
    }

    args.subcmd.run(&mut config).or_else(|err| {
        eprintln!("pk: error: {err}");
        Ok(ExitCode::from(2))
    })
}
