use std::collections::HashSet;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install gettext *.mo files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();
        let dest: PathBuf =
            [d.env.get("DESTTREE").map(|s| s.as_str()).unwrap_or("/usr"), "share/locale"]
                .iter()
                .collect();
        let opts = ["-m0644"];
        let install = d.install().dest(&dest)?.ins_options(opts);

        let (mut dirs, mut files) = (HashSet::<PathBuf>::new(), Vec::<(&Path, PathBuf)>::new());
        let filename = format!("{}.mo", d.env.get("PN").expect("$PN undefined"));

        for path in args.iter().map(Path::new) {
            let dir = match path.file_stem() {
                None => continue,
                Some(v) => Path::new(v).join("LC_MESSAGES"),
            };
            files.push((path, dir.join(&filename)));
            dirs.insert(dir);
        }

        install.dirs(dirs)?;
        install.files(files)?;

        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "domo",
            func: run,
            help: LONG_DOC,
            usage: "domo path/to/mo/file",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};
    use std::{env, fs};

    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    use super::super::assert_invalid_args;
    use super::run as domo;
    use crate::pkgsh::BUILD_DATA;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(domo, &[0]);
        }

        #[test]
        fn creation() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                let prefix = dir.path();
                let src_dir = prefix.join("src");
                fs::create_dir(&src_dir).unwrap();
                env::set_current_dir(&src_dir).unwrap();
                d.borrow_mut().env.insert("ED".into(), prefix.to_str().unwrap().into());
                d.borrow_mut().env.insert("PN".into(), "pkgcraft".into());

                let default = 0o100644;

                fs::File::create("en.mo").unwrap();
                domo(&["en.mo"]).unwrap();
                let path = Path::new("usr/share/locale/en/LC_MESSAGES/pkgcraft.mo");
                let path: PathBuf = [prefix, path].iter().collect();
                let meta = fs::metadata(&path).unwrap();
                let mode = meta.mode();
                assert!(mode == default, "mode {mode:#o} is not default {default:#o}");
            })
        }
    }
}
