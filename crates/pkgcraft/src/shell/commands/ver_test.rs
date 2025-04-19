use scallop::{Error, ExecStatus};

use crate::dep::Version;
use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "ver_test",
    disable_help_flag = true,
    long_about = "Perform comparisons on package version strings."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = true, allow_hyphen_values = true, num_args = 2..=3)]
    args: Vec<String>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let (op, cmp) = match &cmd.args[..] {
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
        _ => unreachable!("invalid args"),
    };

    let ret = match op.as_ref() {
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

make_builtin!("ver_test", ver_test_builtin);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::config::Config;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, ver_test};
    use super::*;

    cmd_scope_tests!("ver_test 1 -lt 2-r1");

    #[test]
    fn invalid_args() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        assert_invalid_cmd(ver_test, &[0, 1, 4]);
    }

    #[test]
    fn overflow() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
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
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        for v in ["a", "1_1", "1-2", ">=1", "~1"] {
            let r = ver_test(&[v, "-eq", v]);
            assert_err_re!(r, format!("invalid version: {v}"));
        }
    }

    #[test]
    fn invalid_op() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
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
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        let inverted_op_map: HashMap<_, _> =
            [("==", "!="), ("!=", "=="), ("<", ">="), (">", "<="), ("<=", ">"), (">=", "<")]
                .into_iter()
                .collect();

        let data = test_data();
        for (expr, (v1, op, v2)) in data.version_toml.compares() {
            temp.create_ebuild(format!("cat/pkg-{v1}"), &[]).unwrap();
            let raw_pkg = repo.get_pkg_raw(format!("cat/pkg-{v1}")).unwrap();
            BuildData::from_raw_pkg(&raw_pkg);

            let inverted_op = op_map[inverted_op_map[op]];
            let op = op_map[op];
            let r = ver_test(&[v1, op, v2]);
            assert_eq!(r.unwrap(), ExecStatus::Success, "failed comparing: {expr}");
            let r = ver_test(&[v1, inverted_op, v2]);
            assert_eq!(
                r.unwrap(),
                ExecStatus::Failure(1),
                "failed comparing inverted: {expr}"
            );

            // test pulling v1 from $PVR
            let r = ver_test(&[op, v2]);
            assert_eq!(r.unwrap(), ExecStatus::Success, "failed comparing: {expr}");
            let r = ver_test(&[inverted_op, v2]);
            assert_eq!(
                r.unwrap(),
                ExecStatus::Failure(1),
                "failed comparing inverted: {expr}"
            );
        }
    }
}
