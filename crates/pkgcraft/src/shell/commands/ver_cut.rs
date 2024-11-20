use std::cmp;
use std::io::Write;

use scallop::{Error, ExecStatus};

use crate::io::stdout;
use crate::shell::get_build_mut;

use super::{make_builtin, parse};

const LONG_DOC: &str = "Output substring from package version string and range arguments.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let pv = get_build_mut().cpv().pv();
    let (range, ver) = match args[..] {
        [range] => (range, pv.as_str()),
        [range, ver] => (range, ver),
        _ => return Err(Error::Base(format!("requires 1 or 2 args, got {}", args.len()))),
    };

    let version_parts = parse::version_split(ver)?;
    let len = version_parts.len();
    let (mut start, mut end) = parse::range(range, len / 2)?;

    // remap indices to array positions
    if start != 0 {
        start = cmp::min(start * 2 - 1, len);
    }
    end = cmp::min(end * 2, len);

    let mut stdout = stdout();
    write!(stdout, "{}", &version_parts[start..end].join(""))?;
    stdout.flush()?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "ver_cut 1-2 - 1.2.3";
make_builtin!("ver_cut", ver_cut_builtin);

#[cfg(test)]
mod tests {
    use scallop::source;

    use crate::config::Config;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::TEST_DATA;

    use super::super::{assert_invalid_args, cmd_scope_tests, ver_cut};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        let (_pool, repo) = TEST_DATA.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        assert_invalid_args(ver_cut, &[0, 3]);
    }

    #[test]
    fn invalid_range() {
        let (_pool, repo) = TEST_DATA.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        for rng in ["-", "-2"] {
            let r = ver_cut(&[rng, "2"]);
            assert!(r.unwrap_err().to_string().contains("invalid range"));
        }

        let r = ver_cut(&["3-2", "1.2.3"]);
        assert_err_re!(r, " is greater than end ");
    }

    #[test]
    fn output() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test1", 0, None).unwrap();

        // invalid PV
        for (rng, ver, expected) in [
            ("1-2", ".1.2.3", "1.2"),
            ("0-2", ".1.2.3", ".1.2"),
            ("2-3", "1.2.3.", "2.3"),
            ("2-", "1.2.3.", "2.3."),
            ("2-4", "1.2.3.", "2.3."),
        ] {
            let raw_pkg = temp.create_raw_pkg("cat/pkg-1.2.3", &[]).unwrap();
            BuildData::from_raw_pkg(&raw_pkg);

            let r = ver_cut(&[rng, ver]).unwrap();
            assert_eq!(stdout().get(), expected);
            assert_eq!(r, ExecStatus::Success);
        }

        // valid PV
        for (rng, ver, expected) in [
            ("1", "1.2.3", "1"),
            ("1-1", "1.2.3", "1"),
            ("1-2", "1.2.3", "1.2"),
            ("2-", "1.2.3", "2.3"),
            ("1-", "1.2.3", "1.2.3"),
            ("3-4", "1.2.3b_alpha4", "3b"),
            ("5", "1.2.3b_alpha4", "alpha"),
            ("0-2", "1.2.3", "1.2"),
            ("2-5", "1.2.3", "2.3"),
            ("4", "1.2.3", ""),
            ("0", "1.2.3", ""),
            ("4-", "1.2.3", ""),
        ] {
            let raw_pkg = temp.create_raw_pkg(format!("cat/pkg-{ver}"), &[]).unwrap();
            BuildData::from_raw_pkg(&raw_pkg);

            let r = ver_cut(&[rng, ver]).unwrap();
            assert_eq!(stdout().get(), expected);
            assert_eq!(r, ExecStatus::Success);

            // test pulling version from $PV
            let r = ver_cut(&[rng]).unwrap();
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

        source::string("VER=$(ver_cut 2-5 1.2.3)").unwrap();
        assert_eq!(scallop::variables::optional("VER").unwrap(), "2.3");

        // test pulling version from $PV
        source::string("VER=$(ver_cut 1-2)").unwrap();
        assert_eq!(scallop::variables::optional("VER").unwrap(), "1.2");
    }
}
