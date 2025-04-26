use std::io::stderr;
use std::process::ExitCode;

use clap::Parser;
use clap_verbosity_flag::{Verbosity, log::LevelFilter};
use pkgcraft::cli::reset_sigpipe;
use pkgcraft::config::Config;
use tracing_log::AsTrace;

use crate::subcmds::Subcommand;

mod format;
mod subcmds;

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    version,
    long_about = None,
    disable_help_subcommand = true,
)]
/// pkgcraft command-line tool
pub(crate) struct Command {
    #[command(flatten)]
    verbosity: Verbosity,

    /// Enable/disable color support
    #[arg(long, value_name = "BOOL", hide_possible_values = true, global = true)]
    color: Option<bool>,

    /// Use a custom config
    #[arg(long, value_name = "PATH", global = true)]
    config: Option<String>,

    // positional
    #[command(subcommand)]
    subcmd: Subcommand,
}

impl Command {
    /// Load a custom config or the system config.
    pub(crate) fn load_config(&self) -> anyhow::Result<Config> {
        let mut config = Config::new("pkgcraft", "");
        if let Some(path) = self.config.as_deref() {
            config.load_path(path)?;
        }
        Ok(config)
    }
}

fn main() -> anyhow::Result<ExitCode> {
    // reset SIGPIPE behavior since rust ignores it by default
    reset_sigpipe();

    let args = Command::parse();

    // custom log event formatter that disables target prefixes by default
    let level = args.verbosity.log_level_filter();
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(level > LevelFilter::Info)
        .without_time()
        .compact();

    // create formatting subscriber that uses stderr
    let mut subscriber = tracing_subscriber::fmt()
        .event_format(format)
        .with_max_level(level.as_trace())
        .with_writer(stderr);

    // forcibly enable or disable subscriber output color
    if let Some(value) = args.color {
        subscriber = subscriber.with_ansi(value);
    }

    // initialize global subscriber
    subscriber.init();

    args.subcmd.run(&args).or_else(|err| {
        eprintln!("pk: error: {err}");
        Ok(ExitCode::from(2))
    })
}
