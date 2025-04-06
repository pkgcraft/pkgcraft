use std::process::ExitCode;
use std::{fs, io};

use camino::Utf8PathBuf;
use clap::{Args, CommandFactory, ValueEnum};
use clap_complete::{generate, generate_to, Shell};

use crate::command;

#[derive(Args)]
#[clap(next_help_heading = "Completion options")]
pub(crate) struct Command {
    /// Target directory for completion files
    #[arg(short, long, exclusive = true)]
    dir: Option<Utf8PathBuf>,

    /// Target shell
    #[arg(required_unless_present = "dir")]
    shell: Option<Shell>,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let mut cmd = command::Command::command();

        if let Some(dir) = &self.dir {
            fs::create_dir_all(dir)?;
            for &shell in Shell::value_variants() {
                generate_to(shell, &mut cmd, "pk", dir)?;
            }
        } else if let Some(shell) = self.shell {
            generate(shell, &mut cmd, "pk", &mut io::stdout());
        }

        Ok(ExitCode::SUCCESS)
    }
}
