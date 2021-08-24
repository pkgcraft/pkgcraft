use std::time::Duration;

use anyhow::{Context, Result};
use clap::{App, AppSettings, Arg, ArgMatches, ArgSettings};
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;

pub mod arcanist {
    tonic::include_proto!("arcanist");
}

use arcanist::arcanist_client::ArcanistClient;
use argparse::{positive_int, str_to_bool};
use settings::Settings;

mod argparse;
mod settings;
mod subcmds;

pub type Client = ArcanistClient<Channel>;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("command-line tool leveraging pkgcraft")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommands(subcmds::register())
        .arg(Arg::new("color")
            .setting(ArgSettings::TakesValue)
            .long("color")
            .value_name("BOOLEAN")
            .validator(str_to_bool)
            .about("toggle colored output"))
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
            .short('c')
            .long("connect")
            .value_name("URL")
            .about("connect to given arcanist instance"))
        .arg(Arg::new("timeout")
            .setting(ArgSettings::TakesValue)
            .long("timeout")
            .value_name("SECONDS")
            .default_value("5")
            .validator(positive_int)
            .about("connection timeout"))
}

fn load_settings() -> Result<(Settings, ArgMatches)> {
    let app = cmd();
    let args = app.get_matches();

    // load config settings and then override them with command-line settings
    let mut settings = Settings::new()?;

    if let Some(color) = args.value_of("color") {
        settings.color = str_to_bool(color)?;
    }

    if args.is_present("debug") {
        settings.debug = true;
    }
    settings.verbosity += args.occurrences_of("verbose") as i32;
    settings.verbosity -= args.occurrences_of("quiet") as i32;

    stderrlog::new()
        .modules([module_path!(), "pkgcraft"])
        .verbosity(args.occurrences_of("verbose") as usize)
        .quiet(settings.verbosity < 0)
        .init()?;

    // load pkgcraft config
    settings.load()?;

    Ok((settings, args))
}

#[tokio::main]
async fn main() -> Result<()> {
    let (mut settings, args) = load_settings()?;
    let socket = args.value_of("socket").map(|s| s.to_string());
    let url = socket.clone().unwrap_or_else(|| "http://[::]".to_string());
    let timeout = args
        .value_of("timeout")
        .unwrap_or_default()
        .parse::<u64>()
        .unwrap();
    let user_agent = format!("{}-{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    let endpoint = Endpoint::from_shared(url)?
        .connect_timeout(Duration::from_secs(timeout))
        .user_agent(user_agent)?;

    // connect to arcanist
    let channel: Channel = match socket {
        Some(socket) => endpoint
            .connect()
            .await
            .context(format!("failed connecting to arcanist: {:?}", &socket))?,
        None => {
            let socket = settings
                .config
                .connect_or_spawn_arcanist(None, Some(timeout))?;
            let error = format!("failed connecting to arcanist: {:?}", &socket);
            endpoint
                .connect_with_connector(service_fn(move |_: Uri| {
                    UnixStream::connect(socket.clone())
                }))
                .await
                .context(error)?
        }
    };

    let mut client: Client = ArcanistClient::new(channel);

    subcmds::run(&args, &mut client, &mut settings).await
}
