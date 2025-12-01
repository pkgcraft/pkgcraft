use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use glob::glob;
use itertools::Itertools;
use scallop::variables::var_to_vec;
use scallop::{Error, ExecStatus};

use super::{TryParseArgs, dodoc::install_docs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "einstalldocs",
    long_about = indoc::indoc! {"
        Installs the files specified by the DOCS and HTML_DOCS variables or a default set
        of files.
    "}
)]
struct Command;

const DOCS_DEFAULTS: &[&str] = &[
    "README*",
    "ChangeLog",
    "AUTHORS",
    "NEWS",
    "TODO",
    "CHANGES",
    "THANKS",
    "BUGS",
    "FAQ",
    "CREDITS",
    "CHANGELOG",
];

/// Determine if a given path contains data or is empty.
fn has_data(recursive: bool, path: &Utf8Path) -> bool {
    if let Ok(m) = fs::metadata(path) {
        m.len() > 0 && (recursive || !m.file_type().is_dir())
    } else {
        false
    }
}

// Perform file expansion on doc strings.
// TODO: replace glob usage with native bash pathname expansion?
// TODO: need to perform word expansion on each string as well
fn expand_docs<S: AsRef<str>>(globs: &[S], force: bool) -> scallop::Result<Vec<Utf8PathBuf>> {
    let mut files = vec![];

    for f in globs.iter().map(|s| s.as_ref()) {
        let paths = glob(f)
            .map_err(|e| Error::Base(format!("invalid docs glob pattern: {f}: {e}")))?;
        let paths: Vec<_> = paths
            .into_iter()
            .map(|r| {
                r.map_err(|e| Error::Base(format!("failed reading docs file: {e}")))
                    .and_then(|p| {
                        Utf8PathBuf::from_path_buf(p)
                            .map_err(|p| Error::Base(format!("invalid unicode path: {p:?}")))
                    })
            })
            .try_collect()?;

        // unmatched patterns cause errors for non-default input
        if force && paths.is_empty() {
            return Err(Error::Base(format!("unmatched docs: {f}")));
        }

        files.extend(paths.into_iter().filter(|p| force || has_data(force, p)));
    }

    Ok(files)
}

/// Install document files defined in a given variable.
pub(crate) fn install_docs_from(var: &str) -> scallop::Result<ExecStatus> {
    let (defaults, destination) = match var {
        "DOCS" => (Some(DOCS_DEFAULTS), ""),
        "HTML_DOCS" => (None, "html"),
        _ => return Err(Error::Base(format!("unknown variable: {var}"))),
    };

    let (recursive, paths) = match (var_to_vec(var), defaults) {
        (Some(v), _) => (true, expand_docs(&v, true)?),
        (_, Some(v)) => (false, expand_docs(v, false)?),
        _ => (false, vec![]),
    };

    if !paths.is_empty() {
        install_docs(recursive, &paths, destination)?;
    }

    Ok(ExecStatus::Success)
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let _cmd = Command::try_parse_args(args)?;

    for var in ["DOCS", "HTML_DOCS"] {
        install_docs_from(var)?;
    }

    Ok(ExecStatus::Success)
}

make_builtin!("einstalldocs", einstalldocs_builtin);

#[cfg(test)]
mod tests {
    use scallop::source;

    use crate::shell::BuildData;
    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::einstalldocs};
    use super::*;

    cmd_scope_tests!("einstalldocs");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(einstalldocs, &[1]);
    }

    #[test]
    fn no_files() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        einstalldocs(&[]).unwrap();
        assert!(file_tree.is_empty());
    }

    #[test]
    fn default_files_empty() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        for f in DOCS_DEFAULTS {
            fs::File::create(f.trim_end_matches('*')).unwrap();
        }
        einstalldocs(&[]).unwrap();
        assert!(file_tree.is_empty());
    }

    #[test]
    fn default_files() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        for f in ["README", "NEWS"] {
            fs::write(f, "data").unwrap();
        }
        einstalldocs(&[]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/NEWS"
            [[files]]
            path = "/usr/share/doc/pkg-1/README"
        "#,
        );
    }

    #[test]
    fn default_files_globs() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        for f in ["README-1", "READMEa"] {
            fs::write(f, "data").unwrap();
        }
        einstalldocs(&[]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/README-1"
            [[files]]
            path = "/usr/share/doc/pkg-1/READMEa"
        "#,
        );
    }

    #[test]
    fn docs_array() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        source::string("DOCS=( NEWS subdir dir/. )").unwrap();
        fs::File::create("NEWS").unwrap();
        fs::create_dir("subdir").unwrap();
        fs::File::create("subdir/README").unwrap();
        fs::create_dir("dir").unwrap();
        fs::File::create("dir/AUTHORS").unwrap();
        einstalldocs(&[]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/AUTHORS"
            [[files]]
            path = "/usr/share/doc/pkg-1/NEWS"
            [[files]]
            path = "/usr/share/doc/pkg-1/subdir/README"
        "#,
        );

        // unmatched file
        source::string("DOCS=( readme )").unwrap();
        let r = einstalldocs(&[]);
        assert_err_re!(r, "^unmatched docs: readme$");
    }

    #[test]
    fn docs_string() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        source::string("DOCS=\"NEWS subdir dir/.\"").unwrap();
        fs::File::create("NEWS").unwrap();
        fs::create_dir("subdir").unwrap();
        fs::File::create("subdir/README").unwrap();
        fs::create_dir("dir").unwrap();
        fs::File::create("dir/AUTHORS").unwrap();
        einstalldocs(&[]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/AUTHORS"
            [[files]]
            path = "/usr/share/doc/pkg-1/NEWS"
            [[files]]
            path = "/usr/share/doc/pkg-1/subdir/README"
        "#,
        );

        // unmatched file
        source::string("DOCS=\"readme\"").unwrap();
        let r = einstalldocs(&[]);
        assert_err_re!(r, "^unmatched docs: readme$");
    }

    #[test]
    fn html_docs_array() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        source::string("HTML_DOCS=( a.html subdir dir/. )").unwrap();
        fs::File::create("a.html").unwrap();
        fs::create_dir("subdir").unwrap();
        fs::File::create("subdir/b.html").unwrap();
        fs::create_dir("dir").unwrap();
        fs::File::create("dir/c.html").unwrap();
        einstalldocs(&[]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/a.html"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/c.html"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/subdir/b.html"
        "#,
        );

        // unmatched file
        source::string("HTML_DOCS=( readme.html )").unwrap();
        let r = einstalldocs(&[]);
        assert_err_re!(r, "^unmatched docs: readme.html$");
    }

    #[test]
    fn html_docs_string() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        source::string("HTML_DOCS=\"a.html subdir dir/.\"").unwrap();
        fs::File::create("a.html").unwrap();
        fs::create_dir("subdir").unwrap();
        fs::File::create("subdir/b.html").unwrap();
        fs::create_dir("dir").unwrap();
        fs::File::create("dir/c.html").unwrap();
        einstalldocs(&[]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/a.html"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/c.html"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/subdir/b.html"
        "#,
        );

        // unmatched file
        source::string("HTML_DOCS=\"readme.html\"").unwrap();
        let r = einstalldocs(&[]);
        assert_err_re!(r, "^unmatched docs: readme.html$");
    }
}
