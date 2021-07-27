use anyhow::Result;
use clap::{App, ArgMatches};

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

pub fn run(subcmd: &str, args: &ArgMatches) -> Result<()> {
    match subcmd {
        "repo" => repo::run(args),
        _ => Ok(()),
    }
}
