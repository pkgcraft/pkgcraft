use scallop::builtins::{output_error_func, Builtin};
use scallop::{Error, Result};

use crate::scallop::BUILD_DATA;

static LONG_DOC: &str = "\
Returns shell true (0) if the first argument (a USE flag name) is enabled, false otherwise. If the
flag name is prefixed with !, returns true if the flag is disabled, and false if it is enabled.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<i32> {
    let (negated, flag) = match args.len() {
        1 => match args[0].starts_with('!') {
            false => (false, args[0]),
            true => (true, &args[0][1..]),
        },
        n => return Err(Error::new(format!("requires 1 arg, got {}", n))),
    };

    BUILD_DATA.with(|d| -> Result<i32> {
        let d = d.borrow();

        if !d.iuse_effective.contains(flag) {
            return Err(Error::new(format!("USE flag {:?} not in IUSE", flag)));
        }

        let ret = d.r#use.contains(flag);
        match negated {
            false => Ok(!ret as i32),
            true => Ok(ret as i32),
        }
    })
}

pub static BUILTIN: Builtin = Builtin {
    name: "use",
    func: run,
    help: LONG_DOC,
    usage: "use flag",
    error_func: Some(output_error_func),
};
