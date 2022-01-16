use std::str::FromStr;

use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::variables::string_value;
use scallop::{Error, Result};

use crate::atom::Version;

static LONG_DOC: &str = "\
Perform version testing as defined in the spec.

Returns 0 if the specified test is true, 1 otherwise.
Returns -1 if an error occurred.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let pvr = string_value("PVR").unwrap_or_else(|| String::from(""));
    let pvr = pvr.as_str();
    let (lhs, op, rhs) = match args.len() {
        2 if pvr.is_empty() => return Err(Error::new("$PVR is undefined")),
        2 => (pvr, args[0], args[1]),
        3 => (args[0], args[1], args[2]),
        n => return Err(Error::new(format!("only accepts 2 or 3 args, got {}", n))),
    };

    let ver_lhs = Version::from_str(lhs)?;
    let ver_rhs = Version::from_str(rhs)?;

    let ret = match op {
        "-eq" => ver_lhs == ver_rhs,
        "-ne" => ver_lhs != ver_rhs,
        "-lt" => ver_lhs < ver_rhs,
        "-gt" => ver_lhs > ver_rhs,
        "-le" => ver_lhs <= ver_rhs,
        "-ge" => ver_lhs >= ver_rhs,
        _ => return Err(Error::new(format!("invalid operator: {:?}", op))),
    };

    Ok(ExecStatus::from(ret))
}

pub static BUILTIN: Builtin = Builtin {
    name: "ver_test",
    func: run,
    help: LONG_DOC,
    usage: "ver_test 1 -lt 2-r1",
    error_func: Some(output_error_func),
};
