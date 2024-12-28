use std::io::stderr;
use std::process::ExitCode;

use clap::Parser;
use clap_verbosity_flag::{log::LevelFilter, Verbosity};
use pkgcraft::cli::reset_sigpipe;
use pkgcraft::config::Config;
use tracing_log::AsTrace;

use crate::subcmds::Subcommand;

#[derive(Parser)]
#[command(version, long_about = None, disable_help_subcommand = true)]
/// pkgcraft command-line tool
pub(crate) struct Command {
    #[command(flatten)]
    verbosity: Verbosity,

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

    pub(super) fn run() -> anyhow::Result<ExitCode> {
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

        tracing_subscriber::fmt()
            .event_format(format)
            .with_max_level(level.as_trace())
            .with_writer(stderr)
            .init();

        args.subcmd.run(&args).or_else(|err| {
            eprintln!("pk: error: {err}");
            Ok(ExitCode::from(2))
        })
    }
}
