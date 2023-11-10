use scallop::ExecStatus;

use crate::shell::write_stdout;

use super::{has, make_builtin};

const LONG_DOC: &str = "The same as has, but also prints the first argument if found.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let ret = has(args)?;
    if bool::from(&ret) {
        write_stdout!("{}", args[0])?;
    }

    Ok(ret)
}

const USAGE: &str = "hasv needle ${haystack}";
make_builtin!("hasv", hasv_builtin, run, LONG_DOC, USAGE, BUILTIN);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::BUILTIN as hasv;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(hasv, &[0]);
    }
}
