use std::cmp;

use scallop::builtins::Builtin;
use scallop::variables::string_value;
use scallop::{Error, Result};

use super::{parse, version_split};

static LONG_DOC: &str = "\
Output substring from package version string and range arguments.

Returns -1 on error.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<i32> {
    let pv = string_value("PV").unwrap_or("");
    let (range, ver) = match args.len() {
        1 => (args[0], pv),
        2 => (args[0], args[1]),
        n => return Err(Error::new(format!("requires 1 or 2 args, got {}", n))),
    };

    let version_parts = version_split(ver);
    let len = version_parts.len();
    let (start, end) = parse::range(range, len / 2)?;
    let start_idx = match start {
        0 => 0,
        n => cmp::min(n * 2 - 1, len),
    };
    let end_idx = cmp::min(end * 2, len);
    println!("{}", &version_parts[start_idx..end_idx].join(""));

    Ok(0)
}

pub static BUILTIN: Builtin = Builtin {
    name: "ver_cut",
    func: run,
    help: LONG_DOC,
    usage: "ver_cut 1-2 - 1.2.3",
    exit_on_error: false,
};
