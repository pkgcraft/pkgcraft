use std::io::stderr;

use clap::Parser;
use clap_verbosity_flag::Verbosity;
use pkgcraft::utils::bounded_thread_pool;
use pkgcruft_git::service::PkgcruftServiceBuilder;
use tracing_log::AsTrace;

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

    /// Parallel jobs to run
    #[arg(short, long, default_value_t = num_cpus::get())]
    jobs: usize,

    /// bind to network socket
    #[arg(short, long, value_name = "IP:port")]
    bind: Option<String>,

    /// Use temporary directory for git repo
    #[arg(short, long)]
    temp: bool,

    /// URI to ebuild git repo
    uri: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Command::parse();

    // custom log event formatter that disables target prefixes by default
    let level = args.verbosity.log_level_filter();
    let format = tracing_subscriber::fmt::format().with_level(true).compact();

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

    // initialize global rayon thread pool
    bounded_thread_pool(args.jobs);

    // initialize service
    let mut service = PkgcruftServiceBuilder::new(&args.uri)
        .jobs(args.jobs)
        .temp(args.temp);

    // override default socket
    if let Some(value) = &args.bind {
        service = service.socket(value);
    }

    // start service
    Ok(service.start().await?)
}
