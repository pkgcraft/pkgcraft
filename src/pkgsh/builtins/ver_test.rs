use std::str::FromStr;

use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::variables::string_value;
use scallop::{Error, Result};

use crate::atom::Version;

static LONG_DOC: &str = "Perform comparisons on package version strings.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let pvr = string_value("PVR").unwrap_or_else(|| String::from(""));
    let pvr = pvr.as_str();
    let (v1, op, v2) = match args.len() {
        2 if pvr.is_empty() => return Err(Error::new("$PVR is undefined")),
        2 => (pvr, args[0], args[1]),
        3 => (args[0], args[1], args[2]),
        n => return Err(Error::new(format!("only accepts 2 or 3 args, got {}", n))),
    };

    let v1 = Version::from_str(v1)?;
    let v2 = Version::from_str(v2)?;

    let ret = match op {
        "-eq" => v1 == v2,
        "-ne" => v1 != v2,
        "-lt" => v1 < v2,
        "-gt" => v1 > v2,
        "-le" => v1 <= v2,
        "-ge" => v1 >= v2,
        _ => return Err(Error::new(format!("invalid operator: {}", op))),
    };

    Ok(ExecStatus::from(ret))
}

pub static BUILTIN: Builtin = Builtin {
    name: "ver_test",
    func: run,
    help: LONG_DOC,
    usage: "ver_test 1 -lt 2-r1",
    error_func: Some(output_error_func),
};

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::super::assert_invalid_args;
    use super::run as ver_test;
    use crate::macros::assert_err_re;

    use rusty_fork::rusty_fork_test;
    use scallop::builtins::ExecStatus;
    use scallop::variables::*;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(ver_test, vec![0, 1, 4]);
            // $PVR not defined
            assert!(ver_test(&["-eq", "1"]).is_err());
        }

        #[test]
        fn overflow() {
            let u64_max: u128 = u64::MAX as u128;
            let (vb, vo) = (format!("{}", u64_max), format!("{}", u64_max + 1));
            for args in [&[&vb, "-eq", &vo], &[&vo, "-eq", &vb]] {
                let r = ver_test(args);
                assert_err_re!(r, format!("^invalid version: .*: {}$", vo));
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
                assert_err_re!(r, format!("^invalid operator: {}$", op));
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
            .iter()
            .cloned()
            .collect();

            let inverted_op_map: HashMap<&str, &str> = [
                ("==", "!="),
                ("!=", "=="),
                ("<", ">="),
                (">", "<="),
                ("<=", ">"),
                (">=", "<"),
            ]
            .iter()
            .cloned()
            .collect();

            let mut pvr = Variable::new("PVR");

            for expr in [
                // simple major versions
                ("0 == 0"),
                ("0 != 1"),
                // equal due to integer coercion and "-r0" being the revision default
                ("0 == 0-r0"),
                ("1 == 01"),
                ("01 == 001"),
                ("1.00 == 1.0"),
                ("1.0100 == 1.010"),
                ("01.01 == 1.01"),
                ("0001.1 == 1.1"),
                ("1.2 == 001.2"),
                ("1.0.2 == 1.0.2-r0"),
                ("1.0.2-r0 == 1.000.2"),
                ("1.000.2 == 1.00.2-r0"),
                ("0-r0 == 0-r00"),
                ("0_beta01 == 0_beta001"),
                ("1.2_pre08-r09 == 1.2_pre8-r9"),
                ("1.010.02 != 1.01.2"),
                // minor versions
                ("0.1 < 0.11"),
                ("0.01 > 0.001"),
                ("1.0 > 1"),
                ("1.0_alpha > 1_alpha"),
                ("1.0_alpha > 1"),
                ("1.0_alpha < 1.0"),
                // version letter suffix
                ("0a < 0b"),
                ("1.1z > 1.1a"),
                // release types
                ("1_alpha < 1_beta"),
                ("1_beta < 1_pre"),
                ("1_pre < 1_rc"),
                ("1_rc < 1"),
                ("1 < 1_p"),
                // release suffix vs non-suffix
                ("1.2.3_alpha < 1.2.3"),
                ("1.2.3_beta < 1.2.3"),
                ("1.2.3_pre < 1.2.3"),
                ("1.2.3_rc < 1.2.3"),
                ("1.2.3_p > 1.2.3"),
                // release suffix version
                ("0_alpha1 < 0_alpha2"),
                ("0_alpha2-r1 > 0_alpha1-r2"),
                ("0_p1 < 0_p2"),
                // last release suffix
                ("0_alpha_rc_p > 0_alpha_rc"),
                // revision
                ("0-r2 > 0-r1"),
                ("1.0.2_pre01-r2 > 1.00.2_pre001-r1"),
                // bound limits
                (&format!("{} < {}", u32::MAX, u64::MAX)),
            ] {
                let v: Vec<&str> = expr.split(' ').collect();
                let (v1, op, v2) = (v[0], op_map[v[1]], v[2]);
                let inverted_op = op_map[inverted_op_map[v[1]]];
                let r = ver_test(&[v1, op, v2]);
                assert_eq!(r.unwrap(), ExecStatus::Success);
                let r = ver_test(&[v1, inverted_op, v2]);
                assert_eq!(r.unwrap(), ExecStatus::Failure);

                // test pulling v1 from $PVR
                pvr.bind(v1, None, None).unwrap();
                let r = ver_test(&[op, v2]);
                assert_eq!(r.unwrap(), ExecStatus::Success);
                let r = ver_test(&[inverted_op, v2]);
                assert_eq!(r.unwrap(), ExecStatus::Failure);
            }
        }
    }
}
