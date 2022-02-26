use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::doexe::run as doexe;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install renamed environment files into /etc/env.d/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, doexe)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "newexe",
            func: run,
            help: LONG_DOC,
            usage: "newexe path/to/executable new_filename",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::{env, fs};

    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    use super::super::assert_invalid_args;
    use super::run as newexe;
    use crate::pkgsh::{write_stdin, BUILD_DATA};

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(newexe, &[0, 1, 3]);
        }

        #[test]
        fn creation() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                env::set_current_dir(&dir).unwrap();
                let prefix = dir.path();
                let src_dir = prefix.join("src");
                fs::create_dir(&src_dir).unwrap();
                env::set_current_dir(&src_dir).unwrap();
                d.borrow_mut().env.insert("ED".into(), prefix.to_str().unwrap().into());

                fs::File::create("bin").unwrap();
                newexe(&["bin", "pkgcraft"]).unwrap();
                let path = prefix.join("pkgcraft");
                assert!(path.exists(), "missing file: {path:?}");

                // re-run using data from stdin
                write_stdin!("pkgcraft");
                newexe(&["-", "pkgcraft"]).unwrap();
                assert_eq!(fs::read_to_string(&path).unwrap(), "pkgcraft");
            })
        }
    }
}
