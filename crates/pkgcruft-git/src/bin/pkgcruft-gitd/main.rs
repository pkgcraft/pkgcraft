use std::io::stderr;
use std::net::SocketAddr;

use anyhow::Context;
use camino::Utf8PathBuf;
use clap::Parser;
use clap_verbosity_flag::{Verbosity, log::LevelFilter};
use pkgcraft::config::Config as PkgcraftConfig;
use tokio::net::{TcpListener, UnixListener};
use tokio_stream::wrappers::{TcpListenerStream, UnixListenerStream};
use tonic::transport::Server;
use tracing_log::AsTrace;

use crate::service::PkgcruftService;

mod service;
mod uds;

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    version,
    long_about = None,
    disable_help_subcommand = true,
)]
/// pkgcruft-git daemon
pub(crate) struct Command {
    #[command(flatten)]
    verbosity: Verbosity,

    /// enable/disable color support
    #[arg(long, value_name = "BOOL", hide_possible_values = true, global = true)]
    color: Option<bool>,

    /// bind to network socket
    #[arg(short, long, value_name = "IP:port")]
    bind: Option<String>,

    /// path to ebuild git repo
    repo: Utf8PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Command::parse();

    // custom log event formatter that disables target prefixes by default
    let level = args.verbosity.log_level_filter();
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(level > LevelFilter::Info)
        .without_time()
        .compact();

    // create formatting subscriber that uses stderr
    let mut subscriber = tracing_subscriber::fmt()
        .event_format(format)
        .with_max_level(level.as_trace())
        .with_writer(stderr);

    // forcibly enable or disable subscriber output color
    if let Some(value) = args.color {
        subscriber = subscriber.with_ansi(value);
    }

    // initialize global subscriber
    subscriber.init();

    // verify target path is a valid ebuild repo
    let mut config = PkgcraftConfig::new("pkgcraft", "");
    config
        .add_repo_path("repo", &args.repo, 0)?
        .into_ebuild()
        .map_err(|repo| anyhow::anyhow!("invalid ebuild repo: {repo}"))?;

    // determine network socket
    let socket = if let Some(value) = &args.bind {
        value.clone()
    } else {
        // default to using unix domain socket
        config.path().run.join("pkgcruft.sock").to_string()
    };

    // start service
    let service = PkgcruftService { repo: args.repo.clone() };
    let server = Server::builder().add_service(pkgcruft_git::Server::new(service));

    match socket.parse::<SocketAddr>() {
        // force unix domain sockets to be absolute paths
        Err(_) if socket.starts_with('/') => {
            uds::verify_socket_path(&socket)?;
            let listener = UnixListener::bind(&socket)
                .context(format!("failed binding to socket: {socket}"))?;
            eprintln!("service listening at: {socket}");
            let incoming = UnixListenerStream::new(listener);
            server.serve_with_incoming(incoming).await?;
        }
        Ok(socket) => {
            let listener = TcpListener::bind(&socket)
                .await
                .context(format!("failed binding to socket: {socket}"))?;
            let addr = listener
                .local_addr()
                .context(format!("invalid local address: {socket}"))?;
            eprintln!("service listening at: {addr}");
            let incoming = TcpListenerStream::new(listener);
            server.serve_with_incoming(incoming).await?
        }
        _ => anyhow::bail!("invalid socket: {socket}"),
    }

    Ok(())
}
