use std::fs::File;
use std::{fs, io};

use scallop::{Error, ExecStatus};
use tempfile::tempdir;

use crate::io::stdin;
use crate::utils::is_single_component;

use super::Builtin;

// Underlying implementation for new* builtins.
pub(super) fn new(args: &[&str], builtin: Builtin) -> scallop::Result<ExecStatus> {
    let [source, name] = args[..] else {
        return Err(Error::Base(format!("requires 2 args, got {}", args.len())));
    };

    // filename can't contain a path separator
    if !is_single_component(name) {
        return Err(Error::Base(format!("invalid filename: {name}")));
    }

    // TODO: create tempdir in $T to avoid cross-fs issues as much as possible
    let tmp_dir =
        tempdir().map_err(|e| Error::Base(format!("failed creating tempdir: {e}")))?;
    let dest = tmp_dir.path().join(name);

    if source == "-" {
        let mut file = File::create(&dest)
            .map_err(|e| Error::Base(format!("failed opening file: {dest:?}: {e}")))?;
        io::copy(&mut stdin(), &mut file).map_err(|e| {
            Error::Base(format!("failed writing stdin to file: {dest:?}: {e}"))
        })?;
    } else {
        fs::copy(source, &dest).map_err(|e| {
            Error::Base(format!("failed copying file {source:?} to {dest:?}: {e}"))
        })?;
    }

    let path = dest
        .to_str()
        .ok_or_else(|| Error::Base(format!("invalid utf8 path: {dest:?}")))?;
    builtin(&[path])
}

#[cfg(test)]
mod tests {
    use crate::test::assert_err_re;

    use super::super::newbin;
    use super::*;

    #[test]
    fn invalid_args() {
        // nonexistent
        let r = new(&["bin", "pkgcraft"], newbin);
        assert_err_re!(r, "^failed copying file \"bin\" .*$");

        // filename contains path separator
        for f in ["bin/pkgcraft", "bin//pkgcraft", "/bin/pkgcraft", "pkgcraft/", "/"] {
            let r = new(&["bin", f], newbin);
            assert_err_re!(r, format!("^invalid filename: {f}$"));
        }
    }
}
