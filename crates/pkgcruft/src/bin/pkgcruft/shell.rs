#![allow(unused)]

use std::path::PathBuf;
use std::{env, fs, io};

use clap::{CommandFactory, ValueEnum};
use clap_complete::Shell;

mod command;
mod options;
mod subcmds;

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = env::args().collect();
    let mut cmd = command::Command::command();

    // generate shell completions
    fs::create_dir_all("shell").expect("failed creating output directory");
    for &shell in Shell::value_variants() {
        clap_complete::generate_to(shell, &mut cmd, "pkgcruft", "shell")?;
    }

    Ok(())
}
