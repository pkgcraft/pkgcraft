use std::io::Write;

use scallop::{Error, ExecStatus};

use crate::io::stdout;
use crate::shell::get_build_mut;

use super::{make_builtin, parse};

const LONG_DOC: &str = "Perform string substitution on package version strings.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let pv = get_build_mut().cpv().pv();
    let (ver, args) = match args.len() {
        n if n < 2 => return Err(Error::Base(format!("requires 2 or more args, got {n}"))),

        // even number of args uses $PV
        n if n % 2 == 0 => (pv.as_str(), args),

        // odd number of args uses the last arg as the version
        _ => {
            let idx = args.len() - 1;
            (args[idx], &args[..idx])
        }
    };

    // split version string into separators and components, note that invalid versions
    // like ".1.2.3" are allowed
    let mut version_parts = parse::version_split(ver)?;
    let len = version_parts.len();

    // iterate over (range, separator) pairs, altering the denoted separators as requested
    let mut args_iter = args.chunks_exact(2);
    while let Some(&[range, sep]) = args_iter.next() {
        let (start, end) = parse::range(range, len / 2)?;
        (start..=end)
            .map(|i| i * 2)
            .take_while(|&i| i < len)
            .for_each(|i| {
                if (i > 0 && i < len - 1) || !version_parts[i].is_empty() {
                    version_parts[i] = sep;
                }
            });
    }

    let mut stdout = stdout();
    write!(stdout, "{}", version_parts.join(""))?;
    stdout.flush()?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "ver_rs 2 - 1.2.3";
make_builtin!("ver_rs", ver_rs_builtin);

#[cfg(test)]
mod tests {
    use scallop::source;

    use crate::config::Config;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_args, cmd_scope_tests, ver_rs};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        let data = test_data();
        let (_pool, repo) = data.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        assert_invalid_args(ver_rs, &[0, 1]);
    }

    #[test]
    fn invalid_range() {
        let data = test_data();
        let (_pool, repo) = data.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        for rng in ["-", "-2"] {
            let r = ver_rs(&[rng, "2", "1.2.3"]);
            assert!(r.unwrap_err().to_string().contains("invalid range"));
        }

        let r = ver_rs(&["3-2", "1", "1.2.3"]);
        assert_err_re!(r, " is greater than end ");
    }

    #[test]
    fn output() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();

        // invalid PV
        for (args, expected) in [
            (vec!["1", "-", ".1.2.3"], ".1-2.3"),
            (vec!["0", "-", ".1.2.3"], "-1.2.3"),
            (vec!["2", ".", "1.2-3"], "1.2.3"),
            (vec!["3-5", "_", "4-6", "-", "a1b2c3d4e5"], "a1b_2-c-3-d4e5"),
        ] {
            let raw_pkg = temp.create_raw_pkg("cat/pkg-1.2.3", &[]).unwrap();
            BuildData::from_raw_pkg(&raw_pkg);

            let r = ver_rs(&args).unwrap();
            assert_eq!(stdout().get(), expected);
            assert_eq!(r, ExecStatus::Success);
        }

        // valid PV
        for (mut args, expected) in [
            (vec!["1", "-", "1.2.3"], "1-2.3"),
            (vec!["2", "-", "1.2.3"], "1.2-3"),
            (vec!["1-2", "-", "1.2.3.4"], "1-2-3.4"),
            (vec!["2-", "-", "1.2.3.4"], "1.2-3-4"),
            (vec!["3", ".", "1.2.3a"], "1.2.3.a"),
            (vec!["2-3", "-", "1.2_alpha4"], "1.2-alpha-4"),
            (vec!["3", "-", "2", "", "1.2.3b_alpha4"], "1.23-b_alpha4"),
            (vec!["0", "-", "1.2.3"], "1.2.3"),
            (vec!["3", ".", "1.2.3"], "1.2.3"),
            (vec!["3-", ".", "1.2.3"], "1.2.3"),
            (vec!["3-5", ".", "1.2.3"], "1.2.3"),
        ] {
            let ver = args.last().unwrap();
            let raw_pkg = temp.create_raw_pkg(format!("cat/pkg-{ver}"), &[]).unwrap();
            BuildData::from_raw_pkg(&raw_pkg);

            let r = ver_rs(&args).unwrap();
            assert_eq!(stdout().get(), expected);
            assert_eq!(r, ExecStatus::Success);

            // test pulling version from $PV
            args.pop();
            let r = ver_rs(&args).unwrap();
            assert_eq!(stdout().get(), expected);
            assert_eq!(r, ExecStatus::Success);
        }
    }

    #[test]
    fn subshell() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test1", 0, None).unwrap();
        let raw_pkg = temp.create_raw_pkg("cat/pkg-1.2.3", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        source::string("VER=$(ver_rs 2 - 1.2.3)").unwrap();
        assert_eq!(scallop::variables::optional("VER").unwrap(), "1.2-3");

        // test pulling version from $PV
        source::string("VER=$(ver_rs 1 -)").unwrap();
        assert_eq!(scallop::variables::optional("VER").unwrap(), "1-2.3");
    }
}
