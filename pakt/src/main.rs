use anyhow::Result;
use clap::{App, AppSettings, Arg, ArgMatches, ErrorKind};

use argparse::str_to_bool;

mod argparse;
mod subcmds;

fn main() -> Result<()> {
    let app = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("command-line tool leveraging pkgcraft")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommands(subcmds::register())
        .arg(
            Arg::new("color")
                .long("color")
                .takes_value(true)
                .value_name("BOOLEAN")
                .validator(str_to_bool)
                .about("toggle colored output"),
        )
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
        );

    let app_ignore_errors = app.clone().setting(AppSettings::IgnoreErrors);

    // determine subcommand being run to use for error output
    let pre_parsed = app_ignore_errors.get_matches();
    let cmd = determine_cmd(&pre_parsed);

    let matches = app.try_get_matches().unwrap_or_else(|e| exit(&cmd, &e, 2));

    // TODO: load config settings and then override them with command-line settings

    if let Some(ref color) = matches.value_of("color") {
        let color = str_to_bool(color)?;
    }

    let debug = matches.is_present("debug");

    let mut verbosity = 0;
    verbosity += matches.occurrences_of("verbose");
    verbosity -= matches.occurrences_of("quiet");

    match matches.subcommand() {
        Some((cmd, args)) => subcmds::run(&cmd, &args),
        _ => Ok(()),
    }
}

// determine full command being run including all subcommands
fn determine_cmd(args: &ArgMatches) -> String {
    let mut args: &ArgMatches = args;
    let mut cmd = vec![env!("CARGO_PKG_NAME")];
    while let Some((subcmd, m)) = args.subcommand() {
        cmd.push(subcmd);
        args = m;
    }
    cmd.join(" ")
}

fn exit(cmd: &str, error: &clap::Error, code: i32) -> ! {
    match error.kind {
        ErrorKind::DisplayHelp => println!("{}", error),
        ErrorKind::DisplayVersion => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        }
        _ => {
            // extract error message without extra cruft -- drop help info
            let msg = format!("{}", error);
            let error = msg.split('\n').next().unwrap();
            println!("{}: {}", cmd, error);
        }
    }

    use std::io::Write;
    let _ = std::io::stdout().lock().flush();
    let _ = std::io::stderr().lock().flush();

    std::process::exit(code)
}
