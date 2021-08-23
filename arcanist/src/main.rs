use std::net::SocketAddr;

use anyhow::{Context, Result};
use clap::{App, Arg};
use futures::TryFutureExt;
use tokio::net::UnixListener;
use tonic::transport::Server;

use service::{ArcanistServer, ArcanistService};
use settings::Settings;

mod service;
mod settings;
mod uds;

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
        )
        .arg(
            Arg::new("socket")
                .long("bind")
                .value_name("IP:port")
                .about("bind to given network socket"),
        );

    let matches = app.get_matches();

    // load config settings and then override them with command-line settings
    let mut settings = Settings::new()?;

    if matches.is_present("debug") {
        settings.debug = true;
    }
    settings.verbosity += matches.occurrences_of("verbose") as i32;
    settings.verbosity -= matches.occurrences_of("quiet") as i32;

    settings.socket = matches.value_of("socket").map(|s| s.to_string());

    // TODO: initialize syslog logger

    // load pkgcraft config
    settings.load()?;

    Ok(settings)
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings = load_settings()?;

    let service = ArcanistService::default();
    let server = Server::builder().add_service(ArcanistServer::new(service));

    // use network socket if configured or unix socket default
    match settings.socket {
        None => {
            let sock_name = format!("{}.sock", env!("CARGO_PKG_NAME"));
            let socket = settings.config.get_socket(&sock_name, true)?;
            let incoming = {
                let listener = UnixListener::bind(&socket)
                    .context(format!("failed binding to socket: {:?}", &socket))?;

                async_stream::stream! {
                    while let item = listener.accept().map_ok(|(st, _)| uds::UnixStream(st)).await {
                        yield item;
                    }
                }
            };

            server.serve_with_incoming(incoming).await?;
        }
        Some(socket) => {
            let socket: SocketAddr = socket.parse().context("invalid network socket")?;
            server.serve(socket).await?
        }
    }

    Ok(())
}
