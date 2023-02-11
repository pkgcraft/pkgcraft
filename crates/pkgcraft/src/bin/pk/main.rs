use std::process::ExitCode;

use clap::Parser;

mod format;
mod subcmds;

#[derive(Debug, Parser)]
#[command(version, long_about = None, disable_help_subcommand = true)]
/// pkgcraft command-line tool
struct Cli {
    #[command(subcommand)]
    subcmd: subcmds::Subcommand,
}

trait Run {
    fn run(self) -> anyhow::Result<ExitCode>;
}

fn main() -> anyhow::Result<ExitCode> {
    let args = Cli::parse();
    args.subcmd.run()
}
