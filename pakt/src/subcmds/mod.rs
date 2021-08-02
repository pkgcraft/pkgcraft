use anyhow::{anyhow, Result};
use clap::ArgMatches;

use crate::settings::Settings;

include!(concat!(env!("OUT_DIR"), "/subcmds/generated.rs"));

pub fn run<S: AsRef<str>>(cmd: S, args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    let cmd = cmd.as_ref();
    match FUNC_MAP.get(cmd) {
        Some(func) => func(args, settings),
        None => Err(anyhow!("unknown subcommand: {:?}", cmd)),
    }
}
