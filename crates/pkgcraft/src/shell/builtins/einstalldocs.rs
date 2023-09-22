use std::fs;
use std::path::{Path, PathBuf};

use glob::glob;
use scallop::builtins::ExecStatus;
use scallop::variables::var_to_vec;
use scallop::Error;

use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcInstall;

use super::{dodoc::install_docs, make_builtin};

const LONG_DOC: &str = "\
Installs the files specified by the DOCS and HTML_DOCS variables or a default set of files.";

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
fn has_data(recursive: bool, path: &Path) -> bool {
    if let Ok(m) = fs::metadata(path) {
        m.len() > 0 && (recursive || !m.file_type().is_dir())
    } else {
        false
    }
}

// Perform file expansion on doc strings.
// TODO: replace glob usage with native bash pathname expansion?
// TODO: need to perform word expansion on each string as well
fn expand_docs<S: AsRef<str>>(globs: &[S], force: bool) -> scallop::Result<Vec<PathBuf>> {
    let mut files = vec![];

    for f in globs.iter().map(|s| s.as_ref()) {
        let paths =
            glob(f).map_err(|e| Error::Base(format!("invalid docs glob pattern: {f}: {e}")))?;
        let paths: scallop::Result<Vec<_>> = paths
            .map(|r| r.map_err(|e| Error::Base(format!("failed reading docs file: {e}"))))
            .collect();
        let paths = paths?;

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
    let (defaults, docdesttree) = match var {
        "DOCS" => (Some(DOCS_DEFAULTS), ""),
        "HTML_DOCS" => (None, "html"),
        _ => return Err(Error::Base(format!("unknown variable: {var}"))),
    };

    let (recursive, paths) = if let Ok(v) = var_to_vec(var) {
        (true, expand_docs(&v, true)?)
    } else if let Some(v) = defaults {
        (false, expand_docs(v, false)?)
    } else {
        (false, vec![])
    };

    if !paths.is_empty() {
        let build = get_build_mut();

        // save original docdesttree value and use custom value
        let orig_docdestree = build.docdesttree.clone();
        build.docdesttree = String::from(docdesttree);

        install_docs(recursive, &paths)?;

        // restore original docdesttree value
        build.docdesttree = orig_docdestree;
    }

    Ok(ExecStatus::Success)
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Base(format!("takes no args, got {}", args.len())));
    }

    for var in ["DOCS", "HTML_DOCS"] {
        install_docs_from(var)?;
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "einstalldocs";
make_builtin!("einstalldocs", einstalldocs_builtin, run, LONG_DOC, USAGE, [("6..", [SrcInstall])]);

#[cfg(test)]
mod tests {
    use scallop::source;

    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as einstalldocs;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(einstalldocs, &[1]);
    }

    #[test]
    fn test_no_files() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        einstalldocs(&[]).unwrap();
        assert!(file_tree.is_empty());
    }

    #[test]
    fn test_default_files_empty() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        for f in DOCS_DEFAULTS {
            fs::File::create(f.trim_end_matches('*')).unwrap();
        }
        einstalldocs(&[]).unwrap();
        assert!(file_tree.is_empty());
    }

    #[test]
    fn test_default_files() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
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
    fn test_default_files_globs() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
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
    fn test_docs_array() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
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
    fn test_docs_string() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
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
    fn test_html_docs_array() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
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
    fn test_html_docs_string() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
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
