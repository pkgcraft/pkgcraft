use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Include or exclude paths for symbol stripping.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let mut d = d.borrow_mut();
        let (set, args) = match args.first() {
            Some(&"-x") => (&mut d.strip_exclude, &args[1..]),
            Some(_) => (&mut d.strip_include, args),
            None => return Err(Error::new("requires 1 or more args, got 0")),
        };

        set.extend(args.iter().map(|s| s.to_string()));
        Ok(ExecStatus::Success)
    })
}

pub static BUILTIN: Builtin = Builtin {
    name: "dostrip",
    func: run,
    help: LONG_DOC,
    usage: "dostrip [-x] /path/to/strip",
    error_func: Some(output_error_func),
};
