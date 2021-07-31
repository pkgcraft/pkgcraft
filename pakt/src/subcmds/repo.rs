use anyhow::{anyhow, Context, Result};
use clap::{App, AppSettings, Arg, ArgMatches};

use crate::settings::Settings;

pub fn cmd() -> App<'static> {
    App::new("repo")
        .about("manage available repos")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .subcommand(
            App::new("add")
                .about("register new repo")
                .arg(Arg::new("name").required(true).about("repo name"))
                .arg(Arg::new("uri").required(true).about("repo location"))
                .arg(Arg::new("sync").short('s').about("sync repo"))
        )
        .subcommand(
            App::new("del")
                .about("unregister repo")
                .arg(Arg::new("repos")
                    .required(true)
                    .takes_value(true)
                    .multiple_values(true)
                    .value_name("REPO")
                    .about("repo names"))
                .arg(Arg::new("clean").short('c').about("remove repo files"))
        )
        .subcommand(
            App::new("sync")
                .about("sync repos")
                .arg(Arg::new("repos")
                    .required(true)
                    .takes_value(true)
                    .multiple_values(true)
                    .value_name("REPO")
                    .about("repo names"))
        )
}

pub fn run(args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    match args.subcommand() {
        Some(("add", m)) => add(m, settings),
        Some(("del", m)) => del(m, settings),
        Some(("sync", m)) => sync(m, settings),
        Some((s, _)) => Err(anyhow!("unknown repo subcommand: {:?}", s)),
        None => list(settings),
    }
}

fn list(settings: &Settings) -> Result<()> {
    settings.config.repos.list()
        .context("failed listing repos")
}

fn add(args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    let name = args.value_of("name").unwrap();
    let uri = args.value_of("uri").unwrap();
    let sync = args.is_present("sync");
    settings.config.repos.add(name, uri, sync)
        .context(format!("failed adding repo: {:?}", name))
}

fn del(args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    let repos: Vec<&str> = args.values_of("repos").unwrap().collect();
    let clean = args.is_present("clean");
    settings.config.repos.del(&repos, clean)
        .context(format!("failed removing repo(s)"))
}

fn sync(args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    let repos: Vec<&str> = args.values_of("repos").unwrap().collect();
    settings.config.repos.sync(&repos)
        .context(format!("failed syncing repo(s)"))
}
