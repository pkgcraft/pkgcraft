use anyhow::{Context, Result};
use clap::{App, Arg};
use tokio::net::{TcpListener, UnixListener};
use tokio_stream::wrappers::{TcpListenerStream, UnixListenerStream};
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

    // use network socket if configured or unix socket default
    match settings.socket {
        None => {
            let sock_name = format!("{}.sock", env!("CARGO_PKG_NAME"));
            let socket = settings.config.get_socket(&sock_name, true)?;
            let listener = UnixListener::bind(&socket)
                .context(format!("failed binding to socket: {:?}", &socket))?;
            warp::serve(routes)
                .run_incoming(UnixListenerStream::new(listener))
                .await;
        }
        Some(socket) => {
            let listener = TcpListener::bind(&socket)
                .await
                .context(format!("failed binding to socket: {:?}", &socket))?;
            warp::serve(routes)
                .run_incoming(TcpListenerStream::new(listener))
                .await;
        }
    }

    Ok(())
}
