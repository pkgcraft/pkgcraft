use std::fs::File;
use std::path::Path;
use std::{fs, io};

use nix::unistd::isatty;
use scallop::builtins::{BuiltinFn, ExecStatus};
use scallop::{Error, Result};
use tempfile::tempdir;

use crate::eapi::Feature;
use crate::pkgsh::BUILD_DATA;

// Underlying implementation for new* builtins.
pub(super) fn new(args: &[&str], func: BuiltinFn) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (source, dest) = match args.len() {
            2 => (args[0], Path::new(args[1])),
            n => return Err(Error::Builtin(format!("requires 2, got {n}"))),
        };

        // filename can't contain a path separator
        if dest.parent() != Some(Path::new("")) {
            return Err(Error::Builtin(format!("invalid filename: {dest:?}")));
        }

        // TODO: create tempdir in $T to avoid cross-fs issues as much as possible
        let tmp_dir =
            tempdir().map_err(|e| Error::Builtin(format!("failed creating tempdir: {e}")))?;
        let dest = tmp_dir.path().join(dest);

        if eapi.has(Feature::NewSupportsStdin) && source == "-" {
            if isatty(0).unwrap_or(false) {
                return Err(Error::Builtin("no input available, stdin is a tty".into()));
            }
            let mut file = File::create(&dest)
                .map_err(|e| Error::Builtin(format!("failed opening file: {dest:?}: {e}")))?;
            io::copy(d.borrow_mut().stdin(), &mut file).map_err(|e| {
                Error::Builtin(format!("failed writing stdin to file: {dest:?}: {e}"))
            })?;
        } else {
            fs::copy(source, &dest).map_err(|e| {
                Error::Builtin(format!("failed copying file {source:?} to {dest:?}: {e}"))
            })?;
        }

        let path = dest.to_str().unwrap();
        func(&[path])
    })
}

#[cfg(test)]
mod tests {
    use rusty_fork::rusty_fork_test;

    use super::super::newbin::run as newbin;
    use super::new;
    use crate::macros::assert_err_re;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            // nonexistent
            let r = new(&["bin", "pkgcraft"], newbin);
            assert_err_re!(r, format!("^failed copying file \"bin\" .*$"));

            // filename contains path separator
            for f in ["bin/pkgcraft", "/bin/pkgcraft", "/"] {
                let r = new(&["bin", f], newbin);
                assert_err_re!(r, format!("^invalid filename: {f:?}$"));
            }
        }
    }
}
