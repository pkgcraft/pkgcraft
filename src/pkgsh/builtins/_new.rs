use std::fs::File;
use std::{fs, io};

use nix::unistd::isatty;
use scallop::builtins::{BuiltinFn, ExecStatus};
use scallop::{Error, Result};
use tempfile::tempdir;

use crate::pkgsh::BUILD_DATA;

// Underlying implementation for new* builtins.
pub(super) fn new(args: &[&str], func: BuiltinFn) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (source, dest) = match args.len() {
            2 => (args[0], args[1]),
            n => return Err(Error::Builtin(format!("requires 2, got {n}"))),
        };

        let tmp_dir =
            tempdir().map_err(|e| Error::Builtin(format!("failed creating tempdir: {e}")))?;
        let dest = tmp_dir.path().join(dest);

        if eapi.has("new_supports_stdin") && source == "-" {
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
