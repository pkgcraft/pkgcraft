use scallop::builtins::{output_error_func, Builtin};
use scallop::{Error, Result};

use crate::scallop::BUILD_DATA;

static LONG_DOC: &str = "\
Returns shell true (0) if the first argument (a USE flag name) is included in IUSE_EFFECTIVE, false
otherwise.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<i32> {
    let flag = match args.len() {
        1 => args[0],
        n => return Err(Error::new(format!("requires 1 arg, got {}", n))),
    };

    BUILD_DATA.with(|d| -> Result<i32> {
        let iuse_effective = &d.borrow().iuse_effective;
        Ok(!iuse_effective.contains(flag) as i32)
    })
}

pub static BUILTIN: Builtin = Builtin {
    name: "in_iuse",
    func: run,
    help: LONG_DOC,
    usage: "in_iuse flag",
    error_func: Some(output_error_func),
};
