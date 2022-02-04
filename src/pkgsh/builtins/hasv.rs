use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::Result;

use super::has;

static LONG_DOC: &str = "The same as has, but also prints the first argument if found.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let ret = has::run(args)?;
    if bool::from(&ret) {
        println!("{}", args[0]);
    }

    Ok(ret)
}

pub static BUILTIN: Builtin = Builtin {
    name: "hasv",
    func: run,
    help: LONG_DOC,
    usage: "hasv needle ${haystack}",
    error_func: Some(output_error_func),
};

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as hasv;

    #[test]
    fn invalid_args() {
        assert_invalid_args(hasv, vec![0]);
    }
}
