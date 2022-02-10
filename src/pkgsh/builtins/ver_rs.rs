use std::io::{stdout, Write};

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::variables::string_value;
use scallop::{Error, Result};

use super::{parse, version_split, PkgBuiltin, ALL};
use crate::macros::write_flush;

static LONG_DOC: &str = "Perform string substitution on package version strings.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let pv = string_value("PV").unwrap_or_else(|| String::from(""));
    let (ver, args) = match args.len() {
        n if n < 2 => {
            return Err(Error::Builtin(format!(
                "requires 2 or more args, got {}",
                n
            )))
        }

        // even number of args uses $PV
        n if n % 2 == 0 => (pv.as_str(), args),

        // odd number of args uses the last arg as the version
        _ => {
            let idx = args.len() - 1;
            (args[idx], &args[..idx])
        }
    };

    // Split version string into separators and components, note that the invalid versions
    // like ".1.2.3" are allowed.
    let mut version_parts = version_split(ver);

    // iterate over (range, separator) pairs
    let mut args_iter = args.chunks_exact(2);
    while let Some(&[range, sep]) = args_iter.next() {
        let len = version_parts.len();
        let (start, end) = parse::range(range, len / 2)?;
        (start..=end)
            .map(|i| i * 2)
            .take_while(|i| i < &len)
            .for_each(|i| {
                if (i > 0 && i < len - 1) || !version_parts[i].is_empty() {
                    version_parts[i] = sep;
                }
            });
    }

    write_flush!(stdout(), "{}", version_parts.join(""));

    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "ver_rs",
            func: run,
            help: LONG_DOC,
            usage: "ver_rs 2 - 1.2.3",
        },
        &[("7-", &[ALL])],
    )
});

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::super::assert_invalid_args;
    use super::run as ver_rs;
    use crate::macros::assert_err_re;

    use gag::BufferRedirect;
    use rusty_fork::rusty_fork_test;
    use scallop::builtins::ExecStatus;
    use scallop::variables::*;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(ver_rs, &[0, 1]);
        }

        #[test]
        fn invalid_range() {
            for rng in ["-", "-2"] {
                let r = ver_rs(&[rng, "2", "1.2.3"]);
                assert!(r.unwrap_err().to_string().contains("invalid range"));
            }

            let r = ver_rs(&["3-2", "1", "1.2.3"]);
            assert_err_re!(r, " is greater than end ");
        }

        #[test]
        fn output() {
            let mut pv = Variable::new("PV");
            let mut buf = BufferRedirect::stdout().unwrap();
            for (mut args, expected) in [
                    (vec!["1", "-", "1.2.3"], "1-2.3"),
                    (vec!["2", "-", "1.2.3"], "1.2-3"),
                    (vec!["1-2", "-", "1.2.3.4"], "1-2-3.4"),
                    (vec!["2-", "-", "1.2.3.4"], "1.2-3-4"),
                    (vec!["2", ".", "1.2-3"], "1.2.3"),
                    (vec!["3", ".", "1.2.3a"], "1.2.3.a"),
                    (vec!["2-3", "-", "1.2_alpha4"], "1.2-alpha-4"),
                    (vec!["3", "-", "2", "", "1.2.3b_alpha4"], "1.23-b_alpha4"),
                    (vec!["3-5", "_", "4-6", "-", "a1b2c3d4e5"], "a1b_2-c-3-d4e5"),
                    (vec!["1", "-", ".1.2.3"], ".1-2.3"),
                    (vec!["0", "-", ".1.2.3"], "-1.2.3"),
                    (vec!["0", "-", "1.2.3"], "1.2.3"),
                    (vec!["3", ".", "1.2.3"], "1.2.3"),
                    (vec!["3-", ".", "1.2.3"], "1.2.3"),
                    (vec!["3-5", ".", "1.2.3"], "1.2.3"),
                    ] {
                let r = ver_rs(args.as_slice()).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(output, expected);
                assert_eq!(r, ExecStatus::Success);

                // test pulling version from $PV
                pv.bind(args.pop().unwrap(), None, None).unwrap();
                let r = ver_rs(args.as_slice()).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                assert_eq!(output, expected);
                assert_eq!(r, ExecStatus::Success);
            }
        }
    }
}
