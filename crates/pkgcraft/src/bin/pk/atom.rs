use std::io;
use std::process::ExitCode;
use std::str::FromStr;

use aho_corasick::AhoCorasick;
use anyhow::{anyhow, bail};
use clap::{ArgGroup, Parser};
use itertools::Itertools;
use pkgcraft::atom::Atom;

#[derive(Parser, Debug)]
#[command(group(
    ArgGroup::new("action")
        .required(true)
        .args(["compare", "format", "intersects", "parse", "sort"])))]
pub(super) struct AtomSubCmd {
    #[arg(short, long)]
    compare: Option<String>,
    #[arg(short, long, value_names = ["FORMAT", "ATOM"], num_args(2))]
    format: Option<Vec<String>>,
    #[arg(short, long, value_name = "ATOM", num_args(2))]
    intersects: Option<Vec<String>>,
    #[arg(short, long, value_name = "ATOM")]
    parse: Option<String>,
    #[arg(short, long, value_name = "ATOM", num_args(1..))]
    sort: Option<Vec<String>>,
}

pub(super) fn main(data: AtomSubCmd) -> anyhow::Result<ExitCode> {
    match &data {
        AtomSubCmd { compare: Some(s), .. } => compare(s),
        AtomSubCmd { format: Some(vals), .. } => format(vals),
        AtomSubCmd { intersects: Some(vals), .. } => intersects(vals),
        AtomSubCmd { parse: Some(s), .. } => parse(s),
        AtomSubCmd { sort: Some(vals), .. } => sort(vals),
        _ => bail!("unhandled atom command option: {data:?}"),
    }
}

fn compare(s: &str) -> anyhow::Result<ExitCode> {
    let (s1, op, s2) = s
        .split_whitespace()
        .collect_tuple()
        .ok_or_else(|| anyhow!("invalid comparison format: {s}"))?;
    let a1 = Atom::from_str(s1)?;
    let a2 = Atom::from_str(s2)?;
    let result = match op {
        "<" => a1 < a2,
        "<=" => a1 <= a2,
        "==" => a1 == a2,
        "!=" => a1 != a2,
        ">=" => a1 >= a2,
        ">" => a1 > a2,
        _ => bail!("invalid operator: {op}"),
    };
    Ok(ExitCode::from(!result as u8))
}

fn format(vals: &[String]) -> anyhow::Result<ExitCode> {
    let (haystack, s) = vals
        .iter()
        .collect_tuple()
        .ok_or_else(|| anyhow!("invalid format args: {vals:?}"))?;
    let a = Atom::from_str(s)?;
    let (mut patterns, mut values) = (vec![], vec![]);
    let ver_default = a.version().map(|v| v.as_str()).unwrap_or_default();
    for (pat, val) in [
        ("{CATEGORY}", a.category().to_string()),
        ("{P}", format!("{}-{}", a.package(), ver_default)),
        ("{PN}", a.package().to_string()),
        ("{PV}", ver_default.to_string()),
    ] {
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
    let (a1, a2) = (Atom::from_str(s1)?, Atom::from_str(s2)?);
    Ok(ExitCode::from(!a1.intersects(&a2) as u8))
}

fn parse(s: &str) -> anyhow::Result<ExitCode> {
    Atom::from_str(s)?;
    Ok(ExitCode::SUCCESS)
}

fn sort(vals: &[String]) -> anyhow::Result<ExitCode> {
    let mut atoms = Vec::<Atom>::new();

    if vals.len() == 1 && vals[0] == "-" {
        for line in io::stdin().lines() {
            for s in line?.split_whitespace() {
                atoms.push(Atom::from_str(s)?);
            }
        }
    } else {
        for s in vals {
            atoms.push(Atom::from_str(s)?);
        }
    }

    atoms.sort();
    println!("{}", atoms.iter().map(|a| a.as_str()).join("\n"));
    Ok(ExitCode::SUCCESS)
}
