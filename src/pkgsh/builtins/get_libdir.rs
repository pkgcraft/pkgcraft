use std::io::Write;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::{PkgBuiltin, ALL};
use crate::pkgsh::utils::get_libdir;
use crate::pkgsh::write_stdout;

static LONG_DOC: &str = "Output the libdir name.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Builtin(format!("takes no args, got {}", args.len())));
    }

    let libdir = get_libdir(Some("lib")).unwrap();
    write_stdout!("{}", libdir);

    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "get_libdir",
            func: run,
            help: LONG_DOC,
            usage: "get_libdir",
        },
        &[("6-", &[ALL])],
    )
});

#[cfg(test)]
mod tests {
    use rusty_fork::rusty_fork_test;
    use scallop::builtins::ExecStatus;
    use scallop::variables::*;

    use super::super::assert_invalid_args;
    use super::run as get_libdir;
    use crate::pkgsh::assert_stdout;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(get_libdir, &[1]);
        }

        #[test]
        fn default() {
            let mut abi_var = Variable::new("ABI");
            for abi in [None, Some(""), Some("abi")] {
                if let Some(val) = abi {
                    abi_var.bind(val, None, None).unwrap();
                }
                assert_eq!(get_libdir(&[]).unwrap(), ExecStatus::Success);
                assert_stdout!("lib");
            }
        }

        #[test]
        fn abi() {
            let mut abi_var = Variable::new("ABI");
            for (abi, libdir) in [
                    ("abi1", "libabi1"),
                    ("abi2", "libabi2"),
                    ] {
                abi_var.bind(abi, None, None).unwrap();
                bind(format!("LIBDIR_{}", abi), libdir, None, None).unwrap();
                assert_eq!(get_libdir(&[]).unwrap(), ExecStatus::Success);
                assert_stdout!(libdir);
            }
        }
    }
}
