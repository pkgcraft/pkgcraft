use scallop::{Error, ExecStatus};

use crate::dep::Version;
use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Perform comparisons on package version strings.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let pvr = get_build_mut().cpv()?.pvr();
    let (lhs, op, rhs) = match args[..] {
        [op, rhs] => (pvr.as_str(), op, rhs),
        [lhs, op, rhs] => (lhs, op, rhs),
        _ => return Err(Error::Base(format!("only accepts 2 or 3 args, got {}", args.len()))),
    };

    let lhs = Version::parse_without_op(lhs)?;
    let rhs = Version::parse_without_op(rhs)?;

    let ret = match op {
        "-eq" => lhs == rhs,
        "-ne" => lhs != rhs,
        "-lt" => lhs < rhs,
        "-gt" => lhs > rhs,
        "-le" => lhs <= rhs,
        "-ge" => lhs >= rhs,
        _ => return Err(Error::Base(format!("invalid operator: {op}"))),
    };

    Ok(ExecStatus::from(ret))
}

const USAGE: &str = "ver_test 1 -lt 2-r1";
make_builtin!("ver_test", ver_test_builtin);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::shell::BuildData;
    use crate::test::TEST_DATA;

    use super::super::{assert_invalid_args, cmd_scope_tests, ver_test};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        let mut config = Config::default();
        let t = config.temp_repo("test1", 0, None).unwrap();
        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        assert_invalid_args(ver_test, &[0, 1, 4]);
    }

    #[test]
    fn overflow() {
        let mut config = Config::default();
        let t = config.temp_repo("test1", 0, None).unwrap();
        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        let u64_max: u128 = u64::MAX as u128;
        let (vb, vo) = (format!("{u64_max}"), format!("{}", u64_max + 1));
        for args in [&[&vb, "-eq", &vo], &[&vo, "-eq", &vb]] {
            let r = ver_test(args);
            assert_err_re!(r, format!("invalid version: {vo}"));
        }
    }

    #[test]
    fn invalid_versions() {
        let mut config = Config::default();
        let t = config.temp_repo("test1", 0, None).unwrap();
        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        for v in ["a", "1_1", "1-2", ">=1", "~1"] {
            let r = ver_test(&[v, "-eq", v]);
            assert_err_re!(r, format!("invalid version: {v}"));
        }
    }

    #[test]
    fn invalid_op() {
        let mut config = Config::default();
        let t = config.temp_repo("test1", 0, None).unwrap();
        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        for op in [">", ">=", "<", "<=", "==", "!="] {
            let r = ver_test(&["1", op, "2"]);
            assert_err_re!(r, format!("^invalid operator: {op}$"));
        }
    }

    #[test]
    fn return_status() {
        let op_map: HashMap<_, _> = [
            ("==", "-eq"),
            ("!=", "-ne"),
            ("<", "-lt"),
            (">", "-gt"),
            ("<=", "-le"),
            (">=", "-ge"),
        ]
        .into_iter()
        .collect();

        let mut config = Config::default();
        let t = config.temp_repo("test1", 0, None).unwrap();

        let inverted_op_map: HashMap<_, _> =
            [("==", "!="), ("!=", "=="), ("<", ">="), (">", "<="), ("<=", ">"), (">=", "<")]
                .into_iter()
                .collect();

        for (expr, (v1, op, v2)) in TEST_DATA.version_toml.compares() {
            let raw_pkg = t.create_raw_pkg(format!("cat/pkg-{v1}"), &[]).unwrap();
            BuildData::from_raw_pkg(&raw_pkg);

            let inverted_op = op_map[inverted_op_map[op]];
            let op = op_map[op];
            let r = ver_test(&[v1, op, v2]);
            assert_eq!(r.unwrap(), ExecStatus::Success, "failed comparing: {expr}");
            let r = ver_test(&[v1, inverted_op, v2]);
            assert_eq!(r.unwrap(), ExecStatus::Failure(1), "failed comparing inverted: {expr}");

            // test pulling v1 from $PVR
            let r = ver_test(&[op, v2]);
            assert_eq!(r.unwrap(), ExecStatus::Success, "failed comparing: {expr}");
            let r = ver_test(&[inverted_op, v2]);
            assert_eq!(r.unwrap(), ExecStatus::Failure(1), "failed comparing inverted: {expr}");
        }
    }
}
