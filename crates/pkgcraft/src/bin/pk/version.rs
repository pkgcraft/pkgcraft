use std::io;
use std::process::ExitCode;
use std::str::FromStr;

use aho_corasick::AhoCorasick;
use anyhow::{anyhow, bail};
use clap::{ArgGroup, Parser};
use itertools::Itertools;
use pkgcraft::atom::Version;

#[derive(Parser, Debug)]
#[command(group(
    ArgGroup::new("action")
        .required(true)
        .args(["compare", "format", "intersects", "parse", "sort"])))]
pub(super) struct VersionSubCmd {
    #[arg(short, long)]
    compare: Option<String>,
    #[arg(short, long, value_names = ["FORMAT", "VERSION"], num_args(2))]
    format: Option<Vec<String>>,
    #[arg(short, long, value_name = "VERSION", num_args(2))]
    intersects: Option<Vec<String>>,
    #[arg(short, long, value_name = "VERSION")]
    parse: Option<String>,
    #[arg(short, long, value_name = "VERSION", num_args(1..))]
    sort: Option<Vec<String>>,
}

pub(super) fn main(data: VersionSubCmd) -> anyhow::Result<ExitCode> {
    match &data {
        VersionSubCmd { compare: Some(s), .. } => compare(s),
        VersionSubCmd { format: Some(vals), .. } => format(vals),
        VersionSubCmd { intersects: Some(vals), .. } => intersects(vals),
        VersionSubCmd { parse: Some(s), .. } => parse(s),
        VersionSubCmd { sort: Some(vals), .. } => sort(vals),
        _ => bail!("unhandled version command option: {data:?}"),
    }
}

fn compare(s: &str) -> anyhow::Result<ExitCode> {
    let (s1, op, s2) = s
        .split_whitespace()
        .collect_tuple()
        .ok_or_else(|| anyhow!("invalid comparison format: {s}"))?;
    let v1 = Version::from_str(s1)?;
    let v2 = Version::from_str(s2)?;
    let result = match op {
        "<" => v1 < v2,
        "<=" => v1 <= v2,
        "==" => v1 == v2,
        "!=" => v1 != v2,
        ">=" => v1 >= v2,
        ">" => v1 > v2,
        _ => bail!("invalid operator: {op}"),
    };
    Ok(ExitCode::from(!result as u8))
}

fn format(vals: &[String]) -> anyhow::Result<ExitCode> {
    let (haystack, s) = vals
        .iter()
        .collect_tuple()
        .ok_or_else(|| anyhow!("invalid format args: {vals:?}"))?;
    let v = Version::from_str(s)?;
    let (mut patterns, mut values) = (vec![], vec![]);
    for (pat, val) in [("{PV}", v.as_str()), ("{REV}", v.revision().as_str())] {
        patterns.push(pat);
        values.push(val);
    }

    let ac = AhoCorasick::new(&patterns);
    let result = ac.replace_all(haystack, &values);
    println!("{result}");
    Ok(ExitCode::SUCCESS)
}

fn intersects(vals: &[String]) -> anyhow::Result<ExitCode> {
    let (s1, s2) = vals
        .iter()
        .collect_tuple()
        .ok_or_else(|| anyhow!("invalid intersects args: {vals:?}"))?;
    let (v1, v2) = (Version::from_str(s1)?, Version::from_str(s2)?);
    Ok(ExitCode::from(!v1.intersects(&v2) as u8))
}

fn parse(s: &str) -> anyhow::Result<ExitCode> {
    Version::from_str(s)?;
    Ok(ExitCode::SUCCESS)
}

fn sort(vals: &[String]) -> anyhow::Result<ExitCode> {
    let mut versions = Vec::<Version>::new();

    if vals.len() == 1 && vals[0] == "-" {
        for line in io::stdin().lines() {
            for s in line?.split_whitespace() {
                versions.push(Version::from_str(s)?);
            }
        }
    } else {
        for s in vals {
            versions.push(Version::from_str(s)?);
        }
    }

    versions.sort();
    println!("{}", versions.iter().map(|a| a.to_string()).join("\n"));
    Ok(ExitCode::SUCCESS)
}
