use anyhow::{anyhow, Result};
use clap::{App, AppSettings, Arg, ArgMatches};

use argparse::str_to_bool;
use settings::Settings;

mod argparse;
mod settings;
mod subcmds;

fn main() -> Result<()> {
    let app = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("command-line tool leveraging pkgcraft")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommands(subcmds::register())
        .arg(
            Arg::new("color")
                .long("color")
                .takes_value(true)
                .value_name("BOOLEAN")
                .validator(str_to_bool)
                .about("toggle colored output"),
        )
        .arg(Arg::new("debug").long("debug").about("enable debug output"))
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .multiple_occurrences(true)
                .about("enable verbose output"),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .multiple_occurrences(true)
                .about("suppress non-error messages"),
        );

    // determine subcommand being run to use for error output
    //let app_ignore_errors = app.clone().setting(AppSettings::IgnoreErrors);
    //let pre_parsed = app_ignore_errors.get_matches();
    //let cmd = determine_cmd(&pre_parsed);

    let matches = app.get_matches();

    // load config settings and then override them with command-line settings
    let mut settings = Settings::new()?;

    if let Some(ref color) = matches.value_of("color") {
        settings.color = str_to_bool(color)?;
    }

    if matches.is_present("debug") {
        settings.debug = true;
    }
    settings.verbosity += matches.occurrences_of("verbose") as i32;
    settings.verbosity -= matches.occurrences_of("quiet") as i32;

    match matches.subcommand() {
        Some((cmd, args)) => subcmds::run(cmd, args, &mut settings),
        _ => Err(anyhow!("missing subcommand")),
    }
}

// determine full command being run including all subcommands
#[allow(dead_code)]
fn determine_cmd(args: &ArgMatches) -> String {
    let mut args: &ArgMatches = args;
    let mut cmd = vec![env!("CARGO_PKG_NAME")];
    while let Some((subcmd, m)) = args.subcommand() {
        cmd.push(subcmd);
        args = m;
    }
    cmd.join(" ")
}
