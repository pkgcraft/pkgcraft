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

    #[command(flatten)]
    color: colorchoice_clap::Color,

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

    // set color choice
    args.color.write_global();

    // create formatting subscriber that uses stderr
    let level = args.verbosity.log_level_filter();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level.as_trace())
        .with_writer(stderr);

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
