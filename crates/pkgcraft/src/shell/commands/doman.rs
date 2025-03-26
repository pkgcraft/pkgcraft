use std::collections::HashSet;
use std::sync::LazyLock;

use camino::{Utf8Path, Utf8PathBuf};
use regex::Regex;
use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Install man pages into /usr/share/man.";

static DETECT_LANG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<name>\w+)\.(?P<lang>[a-z]{2}(_[A-Z]{2})?)$").unwrap());

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let (args, mut lang) = match args {
        [s, files @ ..] => {
            let lang = s.strip_prefix("-i18n=").map(|s| s.trim_matches('"'));
            if lang.is_some() {
                // only the -i18n option was specified
                if files.is_empty() {
                    return Err(Error::Base("missing filename target".to_string()));
                }
                (files, lang)
            } else {
                (args, lang)
            }
        }
        _ => return Err(Error::Base("requires 1 or more args, got 0".to_string())),
    };

    let install = get_build_mut()
        .install()
        .dest("/usr/share/man")?
        .file_options(["-m0644"]);

    let mut dirs = HashSet::new();
    let mut files = vec![];

    for path in args.iter().map(Utf8Path::new) {
        let (mut base, ext) = match (path.file_stem(), path.extension()) {
            (Some(base), Some(ext)) => (base, ext),
            _ => {
                return Err(Error::Base(format!("invalid file target, use `newman`: {path}")))
            }
        };

        if let Some(m) = DETECT_LANG_RE.captures(base) {
            base = m.name("name").unwrap().as_str();
            if lang.is_none() {
                lang = Some(m.name("lang").unwrap().as_str());
            }
        }

        // construct man page subdirectory
        let mut mandir = Utf8PathBuf::from(lang.unwrap_or(""));
        mandir.push(format!("man{ext}"));

        files.push((path, mandir.join(format!("{base}.{ext}"))));
        dirs.insert(mandir);
    }

    install.dirs(dirs)?;
    install.files_map(files)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "doman path/to/man/page";
make_builtin!("doman", doman_builtin, true);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_args, cmd_scope_tests, doman};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(doman, &[0]);

        let _file_tree = FileTree::new();

        // no targets
        let r = doman(&["-i18n=en"]);
        assert_err_re!(r, "^missing filename target$");

        // nonexistent
        let r = doman(&["manpage"]);
        assert_err_re!(r, "^invalid file target, use `newman`: .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        // standard file
        fs::File::create("pkgcraft.1").unwrap();
        doman(&["pkgcraft.1"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/man/man1/pkgcraft.1"
            mode = 0o100644
        "#,
        );

        // -i18n option usage
        doman(&["-i18n=en", "pkgcraft.1"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/man/en/man1/pkgcraft.1"
        "#,
        );

        // -i18n option with empty lang
        for opt in ["-i18n=", "-i18n=\"\""] {
            doman(&[opt, "pkgcraft.1"]).unwrap();
            file_tree.assert(
                r#"
                [[files]]
                path = "/usr/share/man/man1/pkgcraft.1"
            "#,
            );
        }

        // filename lang detection
        for (file, path) in [
            ("pkgcraft.en.1", "en/man1/pkgcraft.1"),
            ("pkgcraft.en_US.1", "en_US/man1/pkgcraft.1"),
        ] {
            fs::File::create(file).unwrap();
            doman(&[file]).unwrap();
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/usr/share/man/{path}"
            "#
            ));
        }

        // -i18n option overrides filename lang
        doman(&["-i18n=zz", "pkgcraft.en.1"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/man/zz/man1/pkgcraft.1"
        "#,
        );

        // -i18n option with empty lang overrides filename lang
        doman(&["-i18n=", "pkgcraft.en.1"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/man/man1/pkgcraft.1"
        "#,
        );
    }
}
