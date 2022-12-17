use scallop::builtins::ExecStatus;

use crate::pkgsh::write_stdout;

use super::{has::run as has, make_builtin, ALL};

const LONG_DOC: &str = "The same as has, but also prints the first argument if found.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let ret = has(args)?;
    if bool::from(&ret) {
        write_stdout!("{}", args[0])?;
    }

    Ok(ret)
}

const USAGE: &str = "hasv needle ${haystack}";
make_builtin!("hasv", hasv_builtin, run, LONG_DOC, USAGE, &[("0..8", &[ALL])]);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as hasv;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(hasv, &[0]);
    }
}
