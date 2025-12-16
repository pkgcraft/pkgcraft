use crate::builtins::make_builtin;
use crate::{ExecStatus, command};

static LONG_DOC: &str = "Stub builtin used for tests.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> crate::Result<ExecStatus> {
    let cmd = command::current_command_string()?;
    assert_eq!(&cmd, r#"scallop 1 2 3 $foo ${bar} """#);
    assert_eq!(args, &["1", "2", "3", ""]);
    let name = command::current_command_name()?;
    assert_eq!(&name, "scallop");
    Ok(ExecStatus::Success)
}

make_builtin!("scallop", scallop_builtin, run, LONG_DOC, "scallop arg1 arg2");

#[cfg(test)]
mod tests {
    use crate::test::assert_err_re;
    use crate::{builtins, source};

    use super::BUILTIN as scallop;

    #[test]
    fn builtin() {
        // register and enable builtin
        builtins::register([scallop]);
        builtins::enable([scallop]).unwrap();

        // verify basic command directly from bash
        assert!(source::string(r#"scallop 1 2 3 $foo ${bar} """#).is_ok());

        // invalid utf-8 in args
        let r = source::string(r#"scallop 1 $'\x02\xc5\xd8' 2"#);
        assert_err_re!(r, "invalid args: invalid utf-8");
    }
}
