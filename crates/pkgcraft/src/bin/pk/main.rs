use std::io::stdin;
use std::process::ExitCode;

use anyhow::bail;
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use is_terminal::IsTerminal;
use pkgcraft::config::Config;
use tracing_log::AsTrace;

mod format;
mod subcmds;

#[derive(Debug, Parser)]
#[command(version, long_about = None, disable_help_subcommand = true)]
/// pkgcraft command-line tool
struct Command {
    #[command(flatten)]
    verbose: Verbosity,
    #[command(subcommand)]
    subcmd: subcmds::Subcommand,
}

trait Run {
    fn run(self, config: &Config) -> anyhow::Result<ExitCode>;
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
    let mut config = Config::new("pkgcraft", "");
    config.load()?;

    let args = Command::parse();

    tracing_subscriber::fmt()
        .with_max_level(args.verbose.log_level_filter().as_trace())
        .init();

    args.subcmd.run(&config).or_else(|e| {
        eprintln!("{e}");
        Ok(ExitCode::from(2))
    })
}
