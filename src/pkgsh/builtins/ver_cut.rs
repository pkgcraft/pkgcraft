use std::cmp;
use std::io::{stdout, Write};

use scallop::builtins::{Builtin, ExecStatus};
use scallop::variables::string_value;
use scallop::{Error, Result};

use super::{parse, version_split};
use crate::macros::write_flush;

static LONG_DOC: &str = "Output substring from package version string and range arguments.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let pv = string_value("PV").unwrap_or_else(|| String::from(""));
    let (range, ver) = match args.len() {
        1 => (args[0], pv.as_str()),
        2 => (args[0], args[1]),
        n => return Err(Error::Builtin(format!("requires 1 or 2 args, got {}", n))),
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
};

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::super::assert_invalid_args;
    use super::run as ver_cut;
    use crate::macros::assert_err_re;

    use gag::BufferRedirect;
    use rusty_fork::rusty_fork_test;
    use scallop::builtins::ExecStatus;
    use scallop::variables::*;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(ver_cut, &[0, 3]);
        }

        #[test]
        fn invalid_range() {
            for rng in ["-", "-2"] {
                let r = ver_cut(&[rng, "2"]);
                assert!(r.unwrap_err().to_string().contains("invalid range"));
            }

            let r = ver_cut(&["3-2", "1.2.3"]);
            assert_err_re!(r, " is greater than end ");
        }

        #[test]
        fn output() {
            let mut pv = Variable::new("PV");
            let mut buf = BufferRedirect::stdout().unwrap();
            for (rng, ver, expected) in [
                    ("1", "1.2.3", "1"),
                    ("1-1", "1.2.3", "1"),
                    ("1-2", "1.2.3", "1.2"),
                    ("2-", "1.2.3", "2.3"),
                    ("1-", "1.2.3", "1.2.3"),
                    ("3-4", "1.2.3b_alpha4", "3b"),
                    ("5", "1.2.3b_alpha4", "alpha"),
                    ("1-2", ".1.2.3", "1.2"),
                    ("0-2", ".1.2.3", ".1.2"),
                    ("2-3", "1.2.3.", "2.3"),
                    ("2-", "1.2.3.", "2.3."),
                    ("2-4", "1.2.3.", "2.3."),
                    ("0-2", "1.2.3", "1.2"),
                    ("2-5", "1.2.3", "2.3"),
                    ("4", "1.2.3", ""),
                    ("0", "1.2.3", ""),
                    ("4-", "1.2.3", ""),
                    ] {
                let r = ver_cut(&[rng, ver]).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(output, expected);
                assert_eq!(r, ExecStatus::Success);

                // test pulling version from $PV
                pv.bind(ver, None, None).unwrap();
                let r = ver_cut(&[rng]).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(output, expected);
                assert_eq!(r, ExecStatus::Success);
            }
        }
    }
}
