use std::cmp;
use std::io::{stdout, Write};

use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::variables::string_value;
use scallop::{Error, Result};

use super::{parse, version_split};
use crate::macros::write_flush;

static LONG_DOC: &str = "\
Output substring from package version string and range arguments.

Returns -1 on error.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let pv = string_value("PV").unwrap_or_else(|| String::from(""));
    let pv = pv.as_str();
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
    write_flush!(stdout(), "{}", &version_parts[start_idx..end_idx].join(""));

    Ok(ExecStatus::Success)
}

pub static BUILTIN: Builtin = Builtin {
    name: "ver_cut",
    func: run,
    help: LONG_DOC,
    usage: "ver_cut 1-2 - 1.2.3",
    error_func: Some(output_error_func),
};

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as ver_cut;

    #[test]
    fn invalid_args() {
        assert_invalid_args(ver_cut, vec![0, 3]);
    }
}
