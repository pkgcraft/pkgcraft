use std::collections::HashSet;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install man pages into /usr/share/man.";

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
        return Err(Error::Builtin("missing filename target".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();
        let eapi = d.eapi;
        let install = d.install().dest("/usr/share/man")?.file_options(["-m0644"]);

        let (mut dirs, mut files) = (HashSet::<PathBuf>::new(), Vec::<(&Path, PathBuf)>::new());

        for path in args.iter().map(Path::new) {
            let (mut base, ext) = match (
                path.file_stem().map(|s| s.to_str()),
                path.extension().map(|s| s.to_str()),
            ) {
                (Some(Some(base)), Some(Some(ext))) => (base, ext),
                _ => {
                    return Err(Error::Builtin(format!(
                        "invalid file target, use `newman`: {path:?}"
                    )))
                }
            };

            if eapi.has("doman_lang_detect") {
                if let Some(m) = DETECT_LANG_RE.captures(base) {
                    base = m.name("name").unwrap().as_str();
                    if lang.is_empty() || !eapi.has("doman_lang_override") {
                        lang = m.name("lang").unwrap().as_str();
                    }
                }
            }

            // construct man page subdirectory
            let mut mandir = PathBuf::from(lang);
            mandir.push(format!("man{ext}"));

            files.push((path, mandir.join(format!("{base}.{ext}"))));
            dirs.insert(mandir);
        }

        install.dirs(dirs)?;
        install.files_map(files)?;

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
    use std::fs;

    use rusty_fork::rusty_fork_test;

    use super::super::assert_invalid_args;
    use super::run as doman;
    use crate::macros::assert_err_re;
    use crate::pkgsh::test::FileTree;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(doman, &[0]);
        }

        #[test]
        fn errors() {
            let _file_tree = FileTree::new();

            // no targets
            let r = doman(&["-i18n=en"]);
            assert_err_re!(r, format!("^missing filename target$"));

            // `newman` target
            let r = doman(&["manpage"]);
            assert_err_re!(r, format!("^invalid file target, use `newman`: .*$"));
        }

        #[test]
        fn creation() {
            let file_tree = FileTree::new();
            let default_mode = 0o100644;

            // standard file
            fs::File::create("pkgcraft.1").unwrap();
            doman(&["pkgcraft.1"]).unwrap();
            file_tree.assert(format!(r#"
                [[files]]
                path = "/usr/share/man/man1/pkgcraft.1"
                mode = {default_mode}
            "#));

            // -i18n option usage
            doman(&["-i18n=en", "pkgcraft.1"]).unwrap();
            file_tree.assert(r#"
                [[files]]
                path = "/usr/share/man/en/man1/pkgcraft.1"
            "#);

            // filename lang detection
            for (file, path) in [
                ("pkgcraft.en.1", "en/man1/pkgcraft.1"),
                ("pkgcraft.en_US.1", "en_US/man1/pkgcraft.1"),
            ] {
                fs::File::create(file).unwrap();
                doman(&[file]).unwrap();
                file_tree.assert(format!(r#"
                    [[files]]
                    path = "/usr/share/man/{path}"
                "#));
            }

            // -i18n option overrides filename lang
            doman(&["-i18n=zz", "pkgcraft.en.1"]).unwrap();
            file_tree.assert(r#"
                [[files]]
                path = "/usr/share/man/zz/man1/pkgcraft.1"
            "#);
        }
    }
}
