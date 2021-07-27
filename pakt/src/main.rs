use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::{App, AppSettings, Arg};

use argparse::BoolArg;

mod subcmds;
mod argparse;

fn main() -> Result<()> {
    let app = App::new("pakt")
        .version(env!("CARGO_PKG_VERSION"))
        .about("command-line tool leveraging pkgcraft")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::VersionlessSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommands(subcmds::register())
        .arg(Arg::new("color")
            .long("color")
            .takes_value(true)
            .value_name("BOOLEAN")
            .validator(BoolArg::parse)
            .about("toggle colored output"))
        .arg(Arg::new("debug")
            .long("debug")
            .about("enable debug output"))
        .arg(Arg::new("verbose")
            .short('v')
            .long("verbose")
            .multiple(true)
            .about("enable verbose output"))
        .arg(Arg::new("quiet")
            .short('q')
            .long("quiet")
            .multiple(true)
            .about("suppress non-error messages"))
    ;

    let matches = app.get_matches();

    // TODO: load config settings and then override them with command-line settings

    let color = matches.value_of_t_or_exit::<BoolArg>("color").is_true();
    let debug = matches.is_present("debug");

    let mut verbosity = 0;
    verbosity += matches.occurrences_of("verbose");
    verbosity -= matches.occurrences_of("quiet");

    match matches.subcommand() {
        Some((cmd, args)) => subcmds::run(&cmd, &args),
        _ => Ok(()),
    }
}
