use std::io::stderr;
use std::net::SocketAddr;
use std::process;
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use clap_verbosity_flag::{Verbosity, log::LevelFilter};
use hyper_util::rt::TokioIo;
use pkgcraft::config::Config as PkgcraftConfig;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;
use tracing_log::AsTrace;
use url::Url;

use crate::subcmds::Subcommand;

mod subcmds;

pub type Client = pkgcruft_git::Client<Channel>;

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    version,
    long_about = None,
    disable_help_subcommand = true,
)]
/// pkgcruft-git client
pub(crate) struct Command {
    #[command(flatten)]
    verbosity: Verbosity,

    /// enable/disable color support
    #[arg(long, value_name = "BOOL", hide_possible_values = true, global = true)]
    color: Option<bool>,

    /// connect to service
    #[arg(short, long, value_name = "URL")]
    connect: Option<String>,

    /// connection timeout
    #[arg(short, long, value_name = "SECONDS", default_value = "5")]
    timeout: u64,

    #[command(subcommand)]
    subcmd: Subcommand,
}

#[tokio::main]
async fn try_main() -> anyhow::Result<()> {
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

    let user_agent = format!("{}-{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
    let config = PkgcraftConfig::new("pkgcraft", "");

    let url = if let Some(url) = &args.connect {
        // convert raw socket arg into url
        match url.parse::<SocketAddr>() {
            Err(_) => url.to_string(),
            Ok(socket) => format!("http://{socket}"),
        }
    } else {
        // use unix domain socket by default if no connection URL is given
        let path = config.path().run.join("pkgcruft.sock");
        pkgcruft_git::connect_or_spawn(&path, Some(args.timeout)).await?
    };

    // connect to service
    let channel: Channel = match Url::parse(&url) {
        Err(_) => {
            let error = format!("failed connecting to service: {url}");
            Endpoint::from_static("http://[::]")
                .user_agent(user_agent)?
                .connect_with_connector(service_fn(move |_: Uri| {
                    let path = url.clone();
                    async {
                        Ok::<_, std::io::Error>(TokioIo::new(UnixStream::connect(path).await?))
                    }
                }))
                .await
                .context(error)?
        }
        Ok(_) => {
            let error = format!("failed connecting to service: {url}");
            Endpoint::from_shared(url)?
                .connect_timeout(Duration::from_secs(args.timeout))
                .user_agent(user_agent)?
                .connect()
                .await
                .context(error)?
        }
    };

    let mut client: Client = pkgcruft_git::Client::new(channel);
    args.subcmd.run(&mut client).await
}

fn main() {
    // extract error message from tonic status responses
    if let Err(error) = try_main() {
        eprintln!("error: {error}\n");
        error
            .chain()
            .skip(1)
            .for_each(|cause| match cause.downcast_ref() {
                Some(e @ tonic::Status { .. }) => eprintln!("caused by: {}", e.message()),
                _ => eprintln!("caused by: {cause}"),
            });
        process::exit(1);
    }
}
