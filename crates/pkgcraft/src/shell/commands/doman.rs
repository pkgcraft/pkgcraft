use std::collections::HashSet;
use std::str::FromStr;
use std::sync::LazyLock;

use camino::Utf8PathBuf;
use regex::Regex;
use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

static DETECT_LANG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<name>\w+)\.(?P<lang>[a-z]{2}(_[A-Z]{2})?)$").unwrap());

#[derive(Debug, Clone)]
struct Lang(String);

impl FromStr for Lang {
    type Err = Error;

    fn from_str(s: &str) -> scallop::Result<Self> {
        if let Some(value) = s.strip_prefix("-i18n=") {
            Ok(Self(value.to_string()))
        } else {
            Err(Error::Base(format!("invalid lang option: {s}")))
        }
    }
}

#[derive(Debug, Clone)]
struct ManPath(Utf8PathBuf);

impl FromStr for ManPath {
    type Err = Error;

    fn from_str(s: &str) -> scallop::Result<Self> {
        if s.strip_prefix("-i18n=").is_some() {
            Err(Error::Base("missing filename target".to_string()))
        } else {
            Ok(Self(s.into()))
        }
    }
}

#[derive(clap::Parser, Debug)]
#[command(
    name = "doman",
    disable_help_flag = true,
    allow_missing_positional = true,
    long_about = "Install man pages into /usr/share/man."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    // TODO: migrate to long option when clap supports single hyphens
    // See https://github.com/clap-rs/clap/issues/2468.
    #[arg(allow_hyphen_values = true)]
    lang: Option<Lang>,

    #[arg(required = true, allow_hyphen_values = true, value_name = "PATH")]
    paths: Vec<ManPath>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;

    let install = get_build_mut()
        .install()
        .dest("/usr/share/man")?
        .file_options(["-m0644"]);

    let mut dirs = HashSet::new();
    let mut files = vec![];
    let mut lang = cmd.lang.as_ref().map(|x| x.0.as_str());

    for path in cmd.paths.iter().map(|x| x.0.as_path()) {
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

make_builtin!("doman", doman_builtin, true);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, doman};

    cmd_scope_tests!("doman path/to/man/page");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(doman, &[0]);

        let _file_tree = FileTree::new();

        // no targets
        let r = doman(&["-i18n=en"]);
        assert_err_re!(r, "missing filename target");

        // nonexistent
        let r = doman(&["manpage"]);
        assert_err_re!(r, "invalid file target, use `newman`: .*");
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
        doman(&["-i18n=", "pkgcraft.1"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/man/man1/pkgcraft.1"
        "#,
        );

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
