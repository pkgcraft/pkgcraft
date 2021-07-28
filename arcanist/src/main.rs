use anyhow::Result;
use clap::{App, AppSettings, Arg};

use settings::Settings;

mod settings;

fn main() -> Result<()> {
    let app = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("package-building daemon leveraging pkgcraft")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
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

    let matches = app.get_matches();

    // load config settings and then override them with command-line settings
    let mut settings = Settings::new()?;

    if matches.is_present("debug") {
        settings.debug = true;
    }
    settings.verbosity += matches.occurrences_of("verbose") as i32;
    settings.verbosity -= matches.occurrences_of("quiet") as i32;

    Ok(())
}
