use anyhow::Result;
use clap::{App, AppSettings, Arg, ArgMatches, ArgSettings};

use crate::Client;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new("add")
        .about("add packages")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .arg(Arg::new("pkgs")
            .setting(ArgSettings::TakesValue)
            .setting(ArgSettings::MultipleValues)
            .setting(ArgSettings::Required)
            .value_name("PKG")
            .about("packages to install"))
}

pub fn run(args: &ArgMatches, _client: &mut Client) -> Result<()> {
    let pkgs: Vec<_> = args.values_of("pkgs").unwrap().collect();
    println!("{:?}", pkgs);
    Ok(())
}
