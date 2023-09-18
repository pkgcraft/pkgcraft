use std::fs::File;
use std::path::Path;
use std::{fs, io};

use scallop::builtins::{BuiltinFn, ExecStatus};
use scallop::Error;
use tempfile::tempdir;

use crate::shell::get_build_mut;

// Underlying implementation for new* builtins.
pub(super) fn new(args: &[&str], func: BuiltinFn) -> scallop::Result<ExecStatus> {
    let (source, name) = match args[..] {
        [source, name] => (source, name),
        _ => return Err(Error::Base(format!("requires 2 args, got {}", args.len()))),
    };

    // filename can't contain a path separator
    if Path::new(name).parent() != Some(Path::new("")) {
        return Err(Error::Base(format!("invalid filename: {name}")));
    }

    // TODO: create tempdir in $T to avoid cross-fs issues as much as possible
    let tmp_dir = tempdir().map_err(|e| Error::Base(format!("failed creating tempdir: {e}")))?;
    let dest = tmp_dir.path().join(name);

    if source == "-" {
        let mut file = File::create(&dest)
            .map_err(|e| Error::Base(format!("failed opening file: {dest:?}: {e}")))?;
        io::copy(get_build_mut().stdin()?, &mut file)
            .map_err(|e| Error::Base(format!("failed writing stdin to file: {dest:?}: {e}")))?;
    } else {
        fs::copy(source, &dest)
            .map_err(|e| Error::Base(format!("failed copying file {source:?} to {dest:?}: {e}")))?;
    }

    let path = dest.to_str().expect("invalid unicode path");
    func(&[path])
}

#[cfg(test)]
mod tests {
    use crate::macros::assert_err_re;

    use super::super::newbin::run as newbin;
    use super::*;

    #[test]
    fn invalid_args() {
        // nonexistent
        for f in ["pkgcraft", "pkgcraft/"] {
            let r = new(&["bin", f], newbin);
            assert_err_re!(r, "^failed copying file \"bin\" .*$");
        }

        // filename contains path separator
        for f in ["bin/pkgcraft", "bin//pkgcraft", "/bin/pkgcraft", "/"] {
            let r = new(&["bin", f], newbin);
            assert_err_re!(r, format!("^invalid filename: {f}$"));
        }
    }
}
