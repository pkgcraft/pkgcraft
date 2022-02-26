use std::cmp;
use std::io::Write;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::variables::string_value;
use scallop::{Error, Result};

use super::{parse, version_split, PkgBuiltin, ALL};
use crate::pkgsh::write_stdout;

const LONG_DOC: &str = "Output substring from package version string and range arguments.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let pv = string_value("PV").unwrap_or_else(|| String::from(""));
    let (range, ver) = match args.len() {
        1 => (args[0], pv.as_str()),
        2 => (args[0], args[1]),
        n => return Err(Error::Builtin(format!("requires 1 or 2 args, got {n}"))),
    };

    let version_parts = version_split(ver);
    let len = version_parts.len();
    let (mut start, mut end) = parse::range(range, len / 2)?;

    // remap indices to array positions
    if start != 0 {
        start = cmp::min(start * 2 - 1, len);
    }
    end = cmp::min(end * 2, len);

    write_stdout!("{}", &version_parts[start..end].join(""));

    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "ver_cut",
            func: run,
            help: LONG_DOC,
            usage: "ver_cut 1-2 - 1.2.3",
        },
        &[("7-", &[ALL])],
    )
});

#[cfg(test)]
mod tests {
    use rusty_fork::rusty_fork_test;
    use scallop::builtins::ExecStatus;
    use scallop::variables::*;

    use super::super::assert_invalid_args;
    use super::run as ver_cut;
    use crate::macros::assert_err_re;
    use crate::pkgsh::assert_stdout;

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
                assert_stdout!(expected);
                assert_eq!(r, ExecStatus::Success);

                // test pulling version from $PV
                pv.bind(ver, None, None).unwrap();
                let r = ver_cut(&[rng]).unwrap();
                assert_stdout!(expected);
                assert_eq!(r, ExecStatus::Success);
            }
        }
    }
}
