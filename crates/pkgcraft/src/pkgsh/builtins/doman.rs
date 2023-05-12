use std::collections::HashSet;
use std::path::{Path, PathBuf};

use regex::Regex;
use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::eapi::Feature;
use crate::pkgsh::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Install man pages into /usr/share/man.";

static DETECT_LANG_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<name>\w+)\.(?P<lang>[a-z]{2}(_[A-Z]{2})?)$").unwrap());

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let (args, mut lang) = match args[0].strip_prefix("-i18n=") {
        None => (args, ""),
        Some(lang) => (&args[1..], lang.trim_matches('"')),
    };

    // only the -i18n option was specified
    if args.is_empty() {
        return Err(Error::Base("missing filename target".into()));
    }

    let eapi = get_build_mut().eapi();
    let install = get_build_mut()
        .install()
        .dest("/usr/share/man")?
        .file_options(["-m0644"]);

    let (mut dirs, mut files) = (HashSet::<PathBuf>::new(), Vec::<(&Path, PathBuf)>::new());

    for path in args.iter().map(Path::new) {
        let (mut base, ext) =
            match (path.file_stem().map(|s| s.to_str()), path.extension().map(|s| s.to_str())) {
                (Some(Some(base)), Some(Some(ext))) => Ok((base, ext)),
                _ => Err(Error::Base(format!("invalid file target, use `newman`: {path:?}"))),
            }?;

        if eapi.has(Feature::DomanLangDetect) {
            if let Some(m) = DETECT_LANG_RE.captures(base) {
                base = m.name("name").unwrap().as_str();
                if lang.is_empty() || !eapi.has(Feature::DomanLangOverride) {
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
}

const USAGE: &str = "doman path/to/man/page";
make_builtin!("doman", doman_builtin, run, LONG_DOC, USAGE, &[("..", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::macros::assert_err_re;
    use crate::pkgsh::test::FileTree;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as doman;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(doman, &[0]);
    }

    #[test]
    fn errors() {
        let _file_tree = FileTree::new();

        // no targets
        let r = doman(&["-i18n=en"]);
        assert_err_re!(r, "^missing filename target$");

        // `newman` target
        let r = doman(&["manpage"]);
        assert_err_re!(r, "^invalid file target, use `newman`: .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o100644;

        // standard file
        fs::File::create("pkgcraft.1").unwrap();
        doman(&["pkgcraft.1"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/share/man/man1/pkgcraft.1"
            mode = {default_mode}
        "#
        ));

        // -i18n option usage
        doman(&["-i18n=en", "pkgcraft.1"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/man/en/man1/pkgcraft.1"
        "#,
        );

        // filename lang detection
        for (file, path) in
            [("pkgcraft.en.1", "en/man1/pkgcraft.1"), ("pkgcraft.en_US.1", "en_US/man1/pkgcraft.1")]
        {
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
    }
}
