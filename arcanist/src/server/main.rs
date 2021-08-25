use std::fs;
use std::net::SocketAddr;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
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
    App::new(env!("CARGO_BIN_NAME"))
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

pub fn get_socket_path(settings: &Settings) -> Result<PathBuf> {
    let socket_dir = &settings.config.path.run;
    let socket = settings.config.path.run.join("arcanist.sock");

    // check if the socket is already in use
    if UnixStream::connect(&socket).is_ok() {
        bail!("arcanist already running on: {:?}", &socket);
    }

    // create dirs and remove old socket file if it exists
    fs::create_dir_all(socket_dir)
        .context(format!("failed creating socket dir: {:?}", socket_dir))?;
    fs::remove_file(&socket).unwrap_or_default();

    Ok(socket)
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings = load_settings()?;
    let mut server = Server::builder();

    // use network socket if configured or unix socket default
    match &settings.socket {
        None => {
            let socket = get_socket_path(&settings)?;
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
