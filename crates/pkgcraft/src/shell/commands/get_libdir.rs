use std::io::Write;

use scallop::ExecStatus;

use crate::io::stdout;
use crate::shell::utils::get_libdir;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(name = "get_libdir", long_about = "Output the libdir name.")]
struct Command;

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let _cmd = Command::try_parse_args(args)?;
    let libdir = get_libdir(Some("lib")).unwrap();
    write!(stdout(), "{libdir}")?;
    Ok(ExecStatus::Success)
}

make_builtin!("get_libdir", get_libdir_builtin);

#[cfg(test)]
mod tests {
    use scallop::variables::*;

    use crate::io::stdout;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, get_libdir};
    use super::*;

    cmd_scope_tests!("get_libdir");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(get_libdir, &[1]);
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
