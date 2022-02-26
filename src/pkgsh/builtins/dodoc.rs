use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install documentation files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (recursive, args) = match args.first() {
            None => return Err(Error::Builtin("requires 1 or more args, got 0".into())),
            Some(&"-r") if eapi.has("dodoc_recursive") => (true, &args[1..]),
            _ => (false, args),
        };

        let dest: PathBuf = [
            "/usr/share/doc",
            d.borrow().env.get("PF").expect("$PF undefined"),
            &d.borrow().docdesttree,
        ]
        .iter()
        .collect();
        let install = d.borrow().install().dest(&dest)?;

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
            name: "dodoc",
            func: run,
            help: LONG_DOC,
            usage: "dodoc [-r] doc_file",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as dodoc;

    #[test]
    fn invalid_args() {
        assert_invalid_args(dodoc, &[0]);
    }
}
