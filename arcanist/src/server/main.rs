use std::fs;
use std::net::SocketAddr;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::{App, Arg, ArgSettings};
use futures::TryFutureExt;
use pkgcraft::config::Config as PkgcraftConfig;
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::RwLock;
use tokio_stream::wrappers::TcpListenerStream;
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
        .arg(Arg::new("config")
            .setting(ArgSettings::TakesValue)
            .long("config")
            .value_name("PATH")
            .about("path to config file"))
}

fn load_settings() -> Result<(Settings, PkgcraftConfig)> {
    let app = cmd();
    let args = app.get_matches();

    // load pkgcraft config
    let config =
        PkgcraftConfig::new("pkgcraft", "", false).context("failed loading pkgcraft config")?;

    // load config settings and then override them with command-line settings
    let config_file = args.value_of("config");
    let mut settings = Settings::new(&config, config_file)?;

    if args.is_present("debug") {
        settings.debug = true;
    }
    settings.verbosity += args.occurrences_of("verbose") as i32;
    settings.verbosity -= args.occurrences_of("quiet") as i32;

    // TODO: initialize syslog logger

    if let Some(socket) = args.value_of("socket") {
        settings.socket = socket.to_string();
    }

    Ok((settings, config))
}

pub fn verify_socket_path(path: String) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    let socket_dir = &path
        .parent()
        .context(format!("invalid socket path: {:?}", &path))?;

    // check if the socket is already in use
    if UnixStream::connect(&path).is_ok() {
        bail!("arcanist already running on: {:?}", &path);
    }

    // create dirs and remove old socket file if it exists
    fs::create_dir_all(socket_dir)
        .context(format!("failed creating socket dir: {:?}", socket_dir))?;
    fs::remove_file(&path).unwrap_or_default();

    Ok(path)
}

#[tokio::main]
async fn main() -> Result<()> {
    let (settings, config) = load_settings()?;

    let socket = match settings.socket.is_empty() {
        false => settings.socket.clone(),
        true => config
            .path
            .run
            .join("arcanist.sock")
            .to_string_lossy()
            .into_owned(),
    };

    let service = ArcanistService {
        settings,
        config: Arc::new(RwLock::new(config)),
    };

    let server = Server::builder().add_service(ArcanistServer::new(service));

    match socket.parse::<SocketAddr>() {
        Err(_) => {
            let socket = verify_socket_path(socket)?;
            let listener = UnixListener::bind(&socket)
                .context(format!("failed binding to socket: {:?}", &socket))?;
            // TODO: log socket that's being used
            let incoming = {
                async_stream::stream! {
                    while let item = listener.accept().map_ok(|(st, _)| uds::UnixStream(st)).await {
                        yield item;
                    }
                }
            };
            server.serve_with_incoming(incoming).await?;
        }
        Ok(socket) => {
            let listener = TcpListener::bind(socket)
                .await
                .context(format!("failed binding to socket: {:?}", &socket))?;
            // TODO: log address that's being used
            let _addr = listener
                .local_addr()
                .context(format!("invalid local address: {:?}", &socket))?;
            let incoming = TcpListenerStream::new(listener);
            server.serve_with_incoming(incoming).await?
        }
    }

    Ok(())
}
