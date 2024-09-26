use std::io::Write;

use scallop::{Error, ExecStatus};

use crate::io::stdout;
use crate::shell::utils::get_libdir;

use super::make_builtin;

const LONG_DOC: &str = "Output the libdir name.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Base(format!("takes no args, got {}", args.len())));
    }

    let libdir = get_libdir(Some("lib")).unwrap();
    write!(stdout(), "{libdir}")?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "get_libdir";
make_builtin!("get_libdir", get_libdir_builtin);

#[cfg(test)]
mod tests {
    use scallop::variables::*;

    use crate::io::stdout;

    use super::super::{assert_invalid_args, cmd_scope_tests, get_libdir};
    use super::*;

    cmd_scope_tests!(USAGE);

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
            assert_eq!(stdout().get(), "lib");
        }
    }

    #[test]
    fn abi() {
        let mut abi_var = Variable::new("ABI");
        for (abi, libdir) in [("abi1", "libabi1"), ("abi2", "libabi2")] {
            abi_var.bind(abi, None, None).unwrap();
            bind(format!("LIBDIR_{abi}"), libdir, None, None).unwrap();
            assert_eq!(get_libdir(&[]).unwrap(), ExecStatus::Success);
            assert_eq!(stdout().get(), libdir);
        }
    }
}
