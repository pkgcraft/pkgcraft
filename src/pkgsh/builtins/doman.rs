use std::collections::HashSet;
use std::path::PathBuf;

use once_cell::sync::Lazy;
use regex::Regex;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Install man pages into /usr/share/man.";

static DETECT_LANG_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<name>\w+)\.(?P<lang>[a-z]{2}(_[A-Z]{2})?)$").unwrap());

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let (args, mut lang) = match args.len() {
        0 => return Err(Error::Builtin("requires 1 or more args, got 0".into())),
        _ => match args[0].strip_prefix("-i18n=") {
            None => (args, ""),
            Some(lang) => (&args[1..], lang.trim_matches('"')),
        },
    };

    // only the -i18n option was specified
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more targets, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();
        let eapi = d.eapi;
        let install = d.install().dest("/usr/share/man")?.ins_options(["-m0644"]);

        let (mut dirs, mut files) = (HashSet::<PathBuf>::new(), Vec::<(&str, PathBuf)>::new());

        for arg in args {
            let (mut basename, ext) = match arg.rsplit_once('.') {
                Some((base, ext)) => (base, ext),
                None => {
                    return Err(Error::Builtin(format!(
                        "invalid file target, use `newman`: {arg:?}"
                    )))
                }
            };

            if eapi.has("doman_lang_detect") {
                if let Some(m) = DETECT_LANG_RE.captures(basename) {
                    basename = m.name("name").unwrap().as_str();
                    if lang.is_empty() || !eapi.has("doman_lang_override") {
                        lang = m.name("lang").unwrap().as_str();
                    }
                }
            }

            // construct man page subdirectory
            let mut mandir = PathBuf::from(lang);
            mandir.push(format!("man{ext}"));

            files.push((arg, mandir.join(format!("{basename}.{ext}"))));
            dirs.insert(mandir);
        }

        install.dirs(dirs)?;
        install.files(files)?;

        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "doman",
            func: run,
            help: LONG_DOC,
            usage: "doman [-i18n=lang] path/to/man/page",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;
    use std::{env, fs};

    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    use super::super::assert_invalid_args;
    use super::run as doman;
    use crate::pkgsh::BUILD_DATA;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(doman, &[0]);
        }

        #[test]
        fn creation() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                let prefix_path = dir.path();
                let prefix = String::from(prefix_path.to_str().unwrap());
                let src_dir = prefix_path.join("src");
                fs::create_dir(&src_dir).unwrap();
                env::set_current_dir(&src_dir).unwrap();
                d.borrow_mut().env.insert("ED".into(), prefix);

                let default = 0o100644;

                // standard file
                fs::File::create("pkgcraft.1").unwrap();
                doman(&["pkgcraft.1"]).unwrap();
                let path = prefix_path.join("usr/share/man/man1/pkgcraft.1");
                let meta = fs::metadata(&path).unwrap();
                let mode = meta.mode();
                assert!(mode == default, "mode {mode:#o} is not default {default:#o}");

                // -i18n option usage
                doman(&["-i18n=en", "pkgcraft.1"]).unwrap();
                let path = prefix_path.join("usr/share/man/en/man1/pkgcraft.1");
                assert!(path.exists(), "missing file: {path:?}");

                // filename lang detection
                for (f, dir) in [
                    ("pkgcraft.en.1", "en/man1/pkgcraft.1"),
                    ("pkgcraft.en_US.1", "en_US/man1/pkgcraft.1"),
                ] {
                    fs::File::create(f).unwrap();
                    doman(&[f]).unwrap();
                    let path = prefix_path.join(format!("usr/share/man/{dir}"));
                    assert!(path.exists(), "missing file: {path:?}");
                }

                // -i18n option overrides filename lang
                doman(&["-i18n=zz", "pkgcraft.en.1"]).unwrap();
                let path = prefix_path.join("usr/share/man/zz/man1/pkgcraft.1");
                assert!(path.exists(), "missing file: {path:?}");
            })
        }
    }
}
