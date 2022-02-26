use std::path::Path;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install files into INSDESTREE.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let (recursive, args) = match args.first() {
        Some(&"-r") => (true, &args[1..]),
        _ => (false, args),
    };

    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more targets, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let dest = &d.borrow().insdesttree;
        let opts = &d.borrow().insopts;
        let install = d.borrow().install().dest(&dest)?.ins_options(opts);

        let (dirs, files): (Vec<&Path>, Vec<&Path>) =
            args.iter().map(Path::new).partition(|p| p.is_dir());

        if !dirs.is_empty() {
            if recursive {
                install.from_dirs(dirs)?;
            } else {
                return Err(Error::Builtin(format!(
                    "trying to install directory as file: {:?}",
                    dirs[0]
                )));
            }
        }

        let files = files
            .into_iter()
            .filter_map(|f| f.file_name().map(|name| (f, name)));
        install.files(files)?;

        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "doins",
            func: run,
            help: LONG_DOC,
            usage: "doins [-r] path/to/file",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as doins;

    #[test]
    fn invalid_args() {
        assert_invalid_args(doins, &[0]);
    }
}
