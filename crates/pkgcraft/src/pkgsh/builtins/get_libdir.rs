use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkgsh::utils::get_libdir;
use crate::pkgsh::write_stdout;

use super::{make_builtin, Scopes::All};

const LONG_DOC: &str = "Output the libdir name.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Base(format!("takes no args, got {}", args.len())));
    }

    let libdir = get_libdir(Some("lib")).unwrap();
    write_stdout!("{libdir}")?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "get_libdir";
make_builtin!("get_libdir", get_libdir_builtin, run, LONG_DOC, USAGE, &[("6..", &[All])]);

#[cfg(test)]
mod tests {
    use scallop::builtins::ExecStatus;
    use scallop::variables::*;

    use crate::pkgsh::assert_stdout;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as get_libdir;
    use super::*;

    builtin_scope_tests!(USAGE);

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
        for (abi, libdir) in [("abi1", "libabi1"), ("abi2", "libabi2")] {
            abi_var.bind(abi, None, None).unwrap();
            bind(format!("LIBDIR_{abi}"), libdir, None, None).unwrap();
            assert_eq!(get_libdir(&[]).unwrap(), ExecStatus::Success);
            assert_stdout!(libdir);
        }
    }
}
