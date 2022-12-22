use scallop::builtins::ExecStatus;
use scallop::{variables, Error};

use crate::pkgsh::write_stdout;

use super::{make_builtin, parse, version_split, ALL};

const LONG_DOC: &str = "Perform string substitution on package version strings.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let pv = variables::optional("PV").unwrap_or_default();
    let (ver, args) = match args.len() {
        n if n < 2 => Err(Error::Base(format!("requires 2 or more args, got {n}"))),

        // even number of args uses $PV
        n if n % 2 == 0 => Ok((pv.as_str(), args)),

        // odd number of args uses the last arg as the version
        _ => {
            let idx = args.len() - 1;
            Ok((args[idx], &args[..idx]))
        }
    }?;

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

    write_stdout!("{}", version_parts.join(""))?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "ver_rs 2 - 1.2.3";
make_builtin!("ver_rs", ver_rs_builtin, run, LONG_DOC, USAGE, &[("7..", &[ALL])]);

#[cfg(test)]
mod tests {
    use scallop::builtins::ExecStatus;
    use scallop::source;
    use scallop::variables::*;

    use crate::macros::assert_err_re;
    use crate::pkgsh::{assert_stdout, BUILD_DATA};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as ver_rs;
    use super::*;

    builtin_scope_tests!(USAGE);

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
            let r = ver_rs(&args).unwrap();
            assert_stdout!(expected);
            assert_eq!(r, ExecStatus::Success);

            // test pulling version from $PV
            pv.bind(args.pop().unwrap(), None, None).unwrap();
            let r = ver_rs(&args).unwrap();
            assert_stdout!(expected);
            assert_eq!(r, ExecStatus::Success);
        }
    }

    #[test]
    fn subshell() {
        BUILD_DATA.with(|d| {
            d.borrow_mut().captured_io = false;
            let ver = Variable::new("VER");

            source::string("VER=$(ver_rs 2 - 1.2.3)").unwrap();
            assert_eq!(ver.optional().unwrap(), "1.2-3");

            // test pulling version from $PV
            source::string("PV=1.2.3; VER=$(ver_rs 1 -)").unwrap();
            assert_eq!(ver.optional().unwrap(), "1-2.3");
        })
    }
}
