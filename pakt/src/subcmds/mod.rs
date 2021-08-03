use anyhow::Result;
use clap::ArgMatches;

use crate::settings::Settings;

include!(concat!(env!("OUT_DIR"), "/subcmds/generated.rs"));

pub fn run(args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    let (subcmd, m) = args.subcommand().unwrap();
    let func = FUNC_MAP.get(subcmd).unwrap();
    func(m, settings)
}
