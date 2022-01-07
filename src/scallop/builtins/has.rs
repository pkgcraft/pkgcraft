use scallop::builtins::Builtin;
use scallop::{Error, Result};

static LONG_DOC: &str = "\
Returns 0 if the first argument is found in the list of subsequent arguments, 1 otherwise.

Returns -1 on error.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<i32> {
    let needle = match args.first() {
        Some(s) => s,
        None => return Err(Error::new("requires 1 or more args")),
    };

    let haystack = &args[1..];
    Ok(!haystack.contains(needle) as i32)
}

pub static BUILTIN: Builtin = Builtin {
    name: "has",
    func: run,
    help: LONG_DOC,
    usage: "has needle ${haystack}",
    exit_on_error: false,
};
