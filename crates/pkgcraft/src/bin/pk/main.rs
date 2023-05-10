use std::io::stdin;
use std::process::ExitCode;

use anyhow::bail;
use clap::Parser;
use is_terminal::IsTerminal;
use pkgcraft::config::Config;

mod format;
mod subcmds;

#[derive(Debug, Parser)]
#[command(version, long_about = None, disable_help_subcommand = true)]
/// pkgcraft command-line tool
struct Command {
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
    // TODO: move repos.conf default locations into pkgcraft
    config.load_repos_conf("/etc/portage/repos.conf")?;

    let args = Command::parse();
    args.subcmd.run(&config).or_else(|e| {
        eprintln!("{e}");
        Ok(ExitCode::from(2))
    })
}
