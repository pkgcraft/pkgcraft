use anyhow::{anyhow, Result};
use clap::{App, ArgMatches};

use crate::settings::Settings;

mod repo;

// combine subcommands from given submodules into a vector for clap
macro_rules! subcmds {
    ($($module:ident),*) => {{
        vec![$($module::cmd(),)*]
    }};
}

// register and return all known subcommands
pub fn register() -> Vec<App<'static>> {
    subcmds!(repo)
}

pub fn run(subcmd: &str, args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    match subcmd {
        "repo" => repo::run(args, settings),
        s => Err(anyhow!("unknown subcommand: {:?}", s)),
    }
}
