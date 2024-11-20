use scallop::{Error, ExecStatus};

use crate::dep::Version;
use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Perform comparisons on package version strings.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let (op, cmp) = match args[..] {
        [op, rhs] => {
            let lhs = get_build_mut().cpv().version();
            let rhs = Version::try_new_without_op(rhs)?;
            (op, lhs.cmp(&rhs))
        }
        [lhs, op, rhs] => {
            let lhs = Version::try_new_without_op(lhs)?;
            let rhs = Version::try_new_without_op(rhs)?;
            (op, lhs.cmp(&rhs))
        }
        _ => return Err(Error::Base(format!("only accepts 2 or 3 args, got {}", args.len()))),
    };

    let ret = match op {
        "-eq" => cmp.is_eq(),
        "-ne" => cmp.is_ne(),
        "-lt" => cmp.is_lt(),
        "-gt" => cmp.is_gt(),
        "-le" => cmp.is_le(),
        "-ge" => cmp.is_ge(),
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
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::TEST_DATA;

    use super::super::{assert_invalid_args, cmd_scope_tests, ver_test};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        let (_pool, repo) = TEST_DATA.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        assert_invalid_args(ver_test, &[0, 1, 4]);
    }

    #[test]
    fn overflow() {
        let (_pool, repo) = TEST_DATA.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
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
        let (_pool, repo) = TEST_DATA.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        for v in ["a", "1_1", "1-2", ">=1", "~1"] {
            let r = ver_test(&[v, "-eq", v]);
            assert_err_re!(r, format!("invalid version: {v}"));
        }
    }

    #[test]
    fn invalid_op() {
        let (_pool, repo) = TEST_DATA.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
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
        let mut temp = config.temp_repo("test", 0, None).unwrap();

        let inverted_op_map: HashMap<_, _> =
            [("==", "!="), ("!=", "=="), ("<", ">="), (">", "<="), ("<=", ">"), (">=", "<")]
                .into_iter()
                .collect();

        for (expr, (v1, op, v2)) in TEST_DATA.version_toml.compares() {
            let raw_pkg = temp.create_raw_pkg(format!("cat/pkg-{v1}"), &[]).unwrap();
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
