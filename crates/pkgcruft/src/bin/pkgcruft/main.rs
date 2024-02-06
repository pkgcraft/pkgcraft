use std::io::stderr;
use std::process::ExitCode;

use clap::Parser;
use clap_verbosity_flag::Verbosity;
use pkgcraft::config::Config;
use scallop::utils::reset_sigpipe;
use tracing_log::AsTrace;

mod args;
mod options;
mod subcmds;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug, Parser)]
#[command(version, long_about = None, disable_help_subcommand = true)]
struct Command {
    #[command(flatten)]
    verbosity: Verbosity,

    // positional
    #[command(subcommand)]
    subcmd: subcmds::Subcommand,
}

impl Command {
    /// Return the vector containing the current running command name including subcommands.
    fn command(&self) -> Vec<&str> {
        let mut cmd = vec![env!("CARGO_BIN_NAME")];
        self.subcmd.command(&mut cmd);
        cmd
    }
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
    config.load()?;

    let cmd = args.command().join(" ");
    args.subcmd.run(&mut config).or_else(|err| {
        eprintln!("{cmd}: error: {err}");
        Ok(ExitCode::from(2))
    })
}
