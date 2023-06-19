use std::str::FromStr;

use scallop::builtins::ExecStatus;
use scallop::{variables, Error};

use crate::dep::Version;

use super::{make_builtin, Scopes::All};

const LONG_DOC: &str = "Perform comparisons on package version strings.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let pvr = variables::optional("PVR").unwrap_or_default();
    let pvr = pvr.as_str();
    let (v1, op, v2) = match args.len() {
        2 if pvr.is_empty() => Err(Error::Base("$PVR is undefined".into())),
        2 => Ok((pvr, args[0], args[1])),
        3 => Ok((args[0], args[1], args[2])),
        n => Err(Error::Base(format!("only accepts 2 or 3 args, got {n}"))),
    }?;

    let v1 = Version::from_str(v1)?;
    let v2 = Version::from_str(v2)?;

    let ret = match op {
        "-eq" => v1 == v2,
        "-ne" => v1 != v2,
        "-lt" => v1 < v2,
        "-gt" => v1 > v2,
        "-le" => v1 <= v2,
        "-ge" => v1 >= v2,
        _ => return Err(Error::Base(format!("invalid operator: {op}"))),
    };

    Ok(ExecStatus::from(ret))
}

const USAGE: &str = "ver_test 1 -lt 2-r1";
make_builtin!("ver_test", ver_test_builtin, run, LONG_DOC, USAGE, &[("7..", &[All])]);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use scallop::builtins::ExecStatus;
    use scallop::variables::*;

    use crate::macros::assert_err_re;
    use crate::test::TEST_DATA;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as ver_test;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(ver_test, &[0, 1, 4]);
        // $PVR not defined
        assert!(ver_test(&["-eq", "1"]).is_err());
    }

    #[test]
    fn overflow() {
        let u64_max: u128 = u64::MAX as u128;
        let (vb, vo) = (format!("{u64_max}"), format!("{}", u64_max + 1));
        for args in [&[&vb, "-eq", &vo], &[&vo, "-eq", &vb]] {
            let r = ver_test(args);
            assert_err_re!(r, format!("^invalid version: .*: {vo}$"));
        }
    }

    #[test]
    fn invalid_versions() {
        for v in ["a", "1_1", "1-2"] {
            let r = ver_test(&[v, "-eq", v]);
            assert!(r.unwrap_err().to_string().contains("invalid version"));
        }
    }

    #[test]
    fn invalid_op() {
        for op in [">", ">=", "<", "<=", "==", "!="] {
            let r = ver_test(&["1", op, "2"]);
            assert_err_re!(r, format!("^invalid operator: {op}$"));
        }
    }

    #[test]
    fn return_status() {
        let op_map: HashMap<&str, &str> = [
            ("==", "-eq"),
            ("!=", "-ne"),
            ("<", "-lt"),
            (">", "-gt"),
            ("<=", "-le"),
            (">=", "-ge"),
        ]
        .into_iter()
        .collect();

        let inverted_op_map: HashMap<&str, &str> =
            [("==", "!="), ("!=", "=="), ("<", ">="), (">", "<="), ("<=", ">"), (">=", "<")]
                .into_iter()
                .collect();

        let mut pvr = Variable::new("PVR");

        for (expr, (v1, op, v2)) in TEST_DATA.version_toml.compares() {
            let inverted_op = op_map[inverted_op_map[op]];
            let op = op_map[op];
            let r = ver_test(&[v1, op, v2]);
            assert_eq!(r.unwrap(), ExecStatus::Success, "failed comparing: {expr}");
            let r = ver_test(&[v1, inverted_op, v2]);
            assert_eq!(r.unwrap(), ExecStatus::Failure(1), "failed comparing: {expr}");

            // test pulling v1 from $PVR
            pvr.bind(v1, None, None).unwrap();
            let r = ver_test(&[op, v2]);
            assert_eq!(r.unwrap(), ExecStatus::Success, "failed comparing: {expr}");
            let r = ver_test(&[inverted_op, v2]);
            assert_eq!(r.unwrap(), ExecStatus::Failure(1), "failed comparing: {expr}");
        }
    }
}
