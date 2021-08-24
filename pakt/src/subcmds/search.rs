use anyhow::Result;
use clap::{App, AppSettings, Arg, ArgMatches, ArgSettings};

use crate::Client;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new("search")
        .about("search repos")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .arg(Arg::new("queries")
            .setting(ArgSettings::TakesValue)
            .setting(ArgSettings::MultipleValues)
            .value_name("TARGET")
            .about("extended atom matching"))
}

pub fn run(args: &ArgMatches, _client: &mut Client) -> Result<()> {
    let queries: Vec<_> = args.values_of("queries").unwrap().collect();
    println!("{:?}", queries);
    Ok(())
}
