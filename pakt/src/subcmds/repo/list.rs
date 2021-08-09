use anyhow::Result;
use clap::{App, ArgMatches};

use crate::settings::Settings;

pub fn cmd() -> App<'static> {
    App::new("list")
        .about("list repos")
        .long_about("List repositories ordered by their priority and then location.")
}

pub fn run(_args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    for (id, config) in settings.config.repos.configs.iter() {
        println!("{}: {:?}", id, config.location);
    }
    Ok(())
}
