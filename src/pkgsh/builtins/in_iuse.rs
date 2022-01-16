use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "\
Returns shell true (0) if the first argument (a USE flag name) is included in IUSE_EFFECTIVE, false
otherwise.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let flag = match args.len() {
        1 => args[0],
        n => return Err(Error::new(format!("requires 1 arg, got {}", n))),
    };

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let iuse_effective = &d.borrow().iuse_effective;
        Ok(ExecStatus::from(iuse_effective.contains(flag)))
    })
}

pub static BUILTIN: Builtin = Builtin {
    name: "in_iuse",
    func: run,
    help: LONG_DOC,
    usage: "in_iuse flag",
    error_func: Some(output_error_func),
};
