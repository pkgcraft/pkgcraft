use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "Include or exclude paths for symbol stripping.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let mut d = d.borrow_mut();
        let (set, args) = match args.first() {
            Some(&"-x") => Ok((&mut d.strip_exclude, &args[1..])),
            Some(_) => Ok((&mut d.strip_include, args)),
            None => Err(Error::Base("requires 1 or more args, got 0".into())),
        }?;

        set.extend(args.iter().map(|s| s.to_string()));
        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "dostrip path/to/strip";
make_builtin!("dostrip", dostrip_builtin, run, LONG_DOC, USAGE, &[("7-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);

    // TODO: add usage tests
}
