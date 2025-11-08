use std::io::stderr;
use std::process::ExitCode;

use clap::Parser;
use clap_verbosity_flag::{Verbosity, log::LevelFilter};
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

    #[command(flatten)]
    color: colorchoice_clap::Color,

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

    // set color choice
    args.color.write_global();

    // custom log event formatter that disables target prefixes by default
    let level = args.verbosity.log_level_filter();
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(level > LevelFilter::Info)
        .without_time()
        .compact();

    // create formatting subscriber that uses stderr
    let subscriber = tracing_subscriber::fmt()
        .event_format(format)
        .with_max_level(level.as_trace())
        .with_writer(stderr);

    // initialize global subscriber
    subscriber.init();

    let cmd = args.cmd().join(" ");
    args.subcmd.run().or_else(|err| {
        eprintln!("{cmd}: error: {err}");
        Ok(ExitCode::from(2))
    })
}
