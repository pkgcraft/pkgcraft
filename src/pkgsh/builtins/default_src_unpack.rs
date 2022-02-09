use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "\
Runs the default src_unpack implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Builtin(format!("takes no args, got {}", args.len())));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let phase_func = &d.borrow().phase_func;
        match eapi.phases().get(phase_func) {
            Some(func) => func(),
            None => Err(Error::Builtin(format!(
                "nonexistent phase function: {}",
                phase_func
            ))),
        }
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "default_src_unpack",
            func: run,
            help: LONG_DOC,
            usage: "default_src_unpack",
        },
        "2-",
        &["src_unpack"],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as default_src_unpack;

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_unpack, &[0]);
    }
}
