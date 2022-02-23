use std::path::Path;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Install environment files into /etc/env.d/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();
        let dest = "/etc/env.d";
        let opts: Vec<&str> = match d.eapi.has("consistent_file_opts") {
            true => vec!["-m0644"],
            false => d.insopts.iter().map(|s| s.as_str()).collect(),
        };
        let install = d.install().dest(&dest)?.ins_options(opts.iter().copied());

        let files = args
            .iter()
            .map(Path::new)
            .filter_map(|f| f.file_name().map(|name| (f, name)));
        install.files(files)?;

        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "doenvd",
            func: run,
            help: LONG_DOC,
            usage: "doenvd path/to/env/file",
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
    use super::super::insopts::run as insopts;
    use super::run as doenvd;
    use crate::eapi::OFFICIAL_EAPIS;
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(doenvd, &[0]);

            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                env::set_current_dir(&dir).unwrap();
                let prefix = dir.path();
                d.borrow_mut().env.insert("ED".into(), prefix.to_str().unwrap().into());

                // nonexistent
                let r = doenvd(&["pkgcraft"]);
                assert_err_re!(r, format!("^invalid file \"pkgcraft\": .*$"));
            })
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

                let default = 0o100644;
                let custom = 0o100755;

                fs::File::create("pkgcraft").unwrap();
                doenvd(&["pkgcraft"]).unwrap();
                let path = Path::new("etc/env.d/pkgcraft");
                let path: PathBuf = [prefix, path].iter().collect();
                let meta = fs::metadata(&path).unwrap();
                let mode = meta.mode();
                assert!(mode == default, "mode {mode:#o} is not default {default:#o}");

                // verify insopts are respected depending on EAPI
                for eapi in OFFICIAL_EAPIS.values() {
                    d.borrow_mut().eapi = eapi;
                    insopts(&["-m0755"]).unwrap();
                    doenvd(&["pkgcraft"]).unwrap();
                    let meta = fs::metadata(&path).unwrap();
                    let mode = meta.mode();
                    if eapi.has("consistent_file_opts") {
                        assert!(mode == default, "mode {mode:#o} is not default {default:#o}");
                    } else {
                        assert!(mode == custom, "mode {mode:#o} is not custom {custom:#o}");
                    }
                }
            })
        }
    }
}
