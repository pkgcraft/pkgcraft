use anyhow::Result;
use clap::{App, Arg};
use warp::Filter;

use settings::Settings;

mod settings;

fn load_settings() -> Result<Settings> {
    let app = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("package-building daemon leveraging pkgcraft")
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

    // TODO: initialize syslog logger

    // load pkgcraft config
    settings.load()?;

    Ok(settings)
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings = load_settings()?;
    let routes =
        warp::any().map(|| format!("{}-{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")));
    warp::serve(routes)
        .run(([127, 0, 0, 1], settings.port))
        .await;
    Ok(())
}
