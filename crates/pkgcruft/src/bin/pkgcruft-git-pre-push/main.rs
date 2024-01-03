use std::io::stderr;
use std::process::ExitCode;

use clap::Parser;
use clap_verbosity_flag::Verbosity;
use pkgcraft::config::Config;
use scallop::utils::reset_sigpipe;
use tracing_log::AsTrace;

#[derive(Debug, Parser)]
#[command(version, long_about = None, disable_help_subcommand = true)]
struct Command {
    #[command(flatten)]
    verbosity: Verbosity,
}

fn main() -> anyhow::Result<ExitCode> {
    // reset SIGPIPE behavior since rust ignores it by default
    reset_sigpipe();

    let args = Command::parse();

    // custom log event formatter
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(false)
        .without_time()
        .compact();

    tracing_subscriber::fmt()
        .event_format(format)
        .with_max_level(args.verbosity.log_level_filter().as_trace())
        .with_writer(stderr)
        .init();

    let _config = Config::new("pkgcraft", "");

    Ok(ExitCode::SUCCESS)
}
