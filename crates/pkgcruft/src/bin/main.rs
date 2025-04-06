use std::io::stderr;
use std::process::ExitCode;

use clap::builder::BoolValueParser;
use clap::Parser;
use clap_verbosity_flag::{log::LevelFilter, Verbosity};
use pkgcraft::cli::reset_sigpipe;
use tracing_log::AsTrace;

use crate::subcmds::Subcommand;

mod options;
mod subcmds;

#[derive(Debug, Parser)]
#[command(version, long_about = None, disable_help_subcommand = true)]
pub(crate) struct Command {
    #[command(flatten)]
    verbosity: Verbosity,

    /// enable/disable color support
    #[arg(
        long,
        global = true,
        value_name = "BOOL",
        num_args = 0..=1,
        default_missing_value = "true",
        value_parser = BoolValueParser::new(),
        hide_possible_values = true,
    )]
    color: Option<bool>,

    // positional
    #[command(subcommand)]
    subcmd: Subcommand,
}

impl Command {
    /// Return the vector containing the current running command name including subcommands.
    fn cmd(&self) -> Vec<&str> {
        let mut cmd = vec![env!("CARGO_BIN_NAME")];
        self.subcmd.command(&mut cmd);
        cmd
    }
}

fn main() -> anyhow::Result<ExitCode> {
    // reset SIGPIPE behavior since rust ignores it by default
    reset_sigpipe();

    let args = Command::parse();

    // ignore the environment and forcibly enable/disable color support
    if let Some(value) = args.color {
        colored::control::set_override(value);
    }

    // custom log event formatter that disables target prefixes by default
    let level = args.verbosity.log_level_filter();
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(level > LevelFilter::Info)
        .without_time()
        .compact();

    tracing_subscriber::fmt()
        .event_format(format)
        .with_ansi(args.color.unwrap_or(true))
        .with_max_level(level.as_trace())
        .with_writer(stderr)
        .init();

    let cmd = args.cmd().join(" ");
    args.subcmd.run().or_else(|err| {
        eprintln!("{cmd}: error: {err}");
        Ok(ExitCode::from(2))
    })
}
