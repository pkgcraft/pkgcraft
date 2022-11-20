use std::fs;
use std::path::{Path, PathBuf};

use glob::glob;
use scallop::builtins::ExecStatus;
use scallop::variables::var_to_vec;
use scallop::Error;

use crate::pkgsh::BUILD_DATA;

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

fn has_data(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(m) => m.len() > 0,
        _ => false,
    }
}

// Perform file expansion on doc strings.
// TODO: replace glob usage with native bash pathname expansion?
// TODO: need to perform word expansion on each string as well
fn expand_docs<S: AsRef<str>>(globs: &[S], force: bool) -> scallop::Result<Vec<PathBuf>> {
    let mut files = vec![];
    // TODO: output warnings for unmatched patterns when running against non-default input
    for f in globs.iter() {
        let paths = glob(f.as_ref()).map_err(|e| Error::Base(e.to_string()))?;
        files.extend(paths.flatten().filter(|p| force || has_data(p)));
    }
    Ok(files)
}

/// Install document files defined in a given variable.
pub(crate) fn install_docs_from(var: &str) -> scallop::Result<ExecStatus> {
    let (defaults, docdesttree) = match var {
        "DOCS" => Ok((Some(DOCS_DEFAULTS), "")),
        "HTML_DOCS" => Ok((None, "html")),
        _ => Err(Error::Base(format!("unknown variable: {var}"))),
    }?;

    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let (recursive, paths) = match var_to_vec(var) {
            Ok(v) => (true, expand_docs(&v, true)?),
            _ => match defaults {
                Some(v) => (false, expand_docs(v, false)?),
                None => (false, vec![]),
            },
        };

        if !paths.is_empty() {
            // save original docdesttree value and use custom value
            let orig_docdestree = d.borrow().docdesttree.clone();
            d.borrow_mut().docdesttree = String::from(docdesttree);

            let paths = paths.iter().map(|p| p.as_path());
            install_docs(recursive, paths)?;

            // restore original docdesttree value
            d.borrow_mut().docdesttree = orig_docdestree;
        }

        Ok(ExecStatus::Success)
    })
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
make_builtin!(
    "einstalldocs",
    einstalldocs_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("6-", &["src_install"])]
);

#[cfg(test)]
mod tests {
    use scallop::source;
    use scallop::variables::bind;

    use crate::pkgsh::test::FileTree;

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
        bind("PF", "pkg-1", None, None).unwrap();
        let file_tree = FileTree::new();
        einstalldocs(&[]).unwrap();
        assert!(file_tree.is_empty());
    }

    #[test]
    fn test_default_files_empty() {
        bind("PF", "pkg-1", None, None).unwrap();
        let file_tree = FileTree::new();
        for f in DOCS_DEFAULTS {
            fs::File::create(f.trim_end_matches('*')).unwrap();
        }
        einstalldocs(&[]).unwrap();
        assert!(file_tree.is_empty());
    }

    #[test]
    fn test_default_files() {
        bind("PF", "pkg-1", None, None).unwrap();
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
        bind("PF", "pkg-1", None, None).unwrap();
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
        bind("PF", "pkg-1", None, None).unwrap();
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
    }

    #[test]
    fn test_docs_string() {
        bind("PF", "pkg-1", None, None).unwrap();
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
    }

    #[test]
    fn test_html_docs_array() {
        bind("PF", "pkg-1", None, None).unwrap();
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
    }

    #[test]
    fn test_html_docs_string() {
        bind("PF", "pkg-1", None, None).unwrap();
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
    }
}
