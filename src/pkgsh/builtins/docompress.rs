use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Include or exclude paths for compression.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let mut d = d.borrow_mut();
        let (set, args) = match args.first() {
            Some(&"-x") => (&mut d.compress_exclude, &args[1..]),
            Some(_) => (&mut d.compress_include, args),
            None => return Err(Error::new("requires 1 or more args, got 0")),
        };

        set.extend(args.iter().map(|s| s.to_string()));
        Ok(ExecStatus::Success)
    })
}

pub static BUILTIN: Builtin = Builtin {
    name: "docompress",
    func: run,
    help: LONG_DOC,
    usage: "docompress [-x] /path/to/compress",
    error_func: Some(output_error_func),
};
