use std::io::{stderr, stdin, Write};
use std::process::ExitCode;

use anyhow::bail;
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use is_terminal::IsTerminal;
use pkgcraft::config::Config;
use scallop::utils::reset_sigpipe;
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
    #[command(subcommand)]
    subcmd: subcmds::Subcommand,
}

trait StdinArgs {
    fn stdin_args(&self) -> anyhow::Result<bool>;
}

impl StdinArgs for Vec<String> {
    fn stdin_args(&self) -> anyhow::Result<bool> {
        match self.iter().next().map(|s| s.as_str()) {
            Some("-") | None => {
                if stdin().is_terminal() {
                    bail!("missing input on stdin");
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

fn main() -> anyhow::Result<ExitCode> {
    // reset SIGPIPE behavior since rust ignores it by default
    reset_sigpipe();

    let mut config = Config::new("pkgcraft", "");
    config.load()?;

    let args = Command::parse();

    // custom log event formatter
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(false)
        .without_time()
        .compact();

    tracing_subscriber::fmt()
        .with_max_level(args.verbosity.log_level_filter().as_trace())
        .event_format(format)
        .init();

    args.subcmd.run(&config).or_else(|err| {
        writeln!(stderr(), "pk: error: {err}").ok();
        Ok(ExitCode::from(2))
    })
}
