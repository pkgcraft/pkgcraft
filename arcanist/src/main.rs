use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{App, Arg, ArgSettings};
use futures::TryFutureExt;
use tokio::net::UnixListener;
use tokio::sync::RwLock;
use tonic::transport::Server;

use crate::service::{ArcanistServer, ArcanistService};
use crate::settings::Settings;

mod service;
mod settings;
mod uds;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("package-building daemon leveraging pkgcraft")
        .arg(Arg::new("debug")
            .long("debug")
            .about("enable debug output"))
        .arg(Arg::new("verbose")
            .setting(ArgSettings::MultipleOccurrences)
            .short('v')
            .long("verbose")
            .about("enable verbose output"))
        .arg(Arg::new("quiet")
            .setting(ArgSettings::MultipleOccurrences)
            .short('q')
            .long("quiet")
            .about("suppress non-error messages"))
        .arg(Arg::new("socket")
            .setting(ArgSettings::TakesValue)
            .long("bind")
            .value_name("IP:port")
            .about("bind to given network socket"))
}

fn load_settings() -> Result<Settings> {
    let app = cmd();
    let args = app.get_matches();

    // load config settings and then override them with command-line settings
    let mut settings = Settings::new()?;

    if args.is_present("debug") {
        settings.debug = true;
    }
    settings.verbosity += args.occurrences_of("verbose") as i32;
    settings.verbosity -= args.occurrences_of("quiet") as i32;

    // TODO: initialize syslog logger

    // load pkgcraft config
    settings.load()?;

    settings.socket = args.value_of("socket").map(|s| s.to_string());

    Ok(settings)
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings = load_settings()?;
    let mut server = Server::builder();

    // use network socket if configured or unix socket default
    match &settings.socket {
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

            let service = ArcanistService {
                settings: Arc::new(RwLock::new(settings)),
            };
            server
                .add_service(ArcanistServer::new(service))
                .serve_with_incoming(incoming)
                .await?;
        }
        Some(socket) => {
            let socket: SocketAddr = socket.parse().context("invalid network socket")?;
            let service = ArcanistService {
                settings: Arc::new(RwLock::new(settings)),
            };
            server
                .add_service(ArcanistServer::new(service))
                .serve(socket)
                .await?
        }
    }

    Ok(())
}
