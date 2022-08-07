use std::collections::HashSet;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Parser;
use scallop::builtins::ExecStatus;
use scallop::{Error, Result};
use walkdir::DirEntry;

use crate::macros::build_from_paths;
use crate::pkgsh::{write_stderr, BUILD_DATA};

use super::make_builtin;

const LONG_DOC: &str = "Install HTML documentation files.";

#[derive(Parser, Debug, Default)]
#[clap(name = "dohtml")]
struct Options {
    #[clap(short = 'r')]
    recursive: bool,
    #[clap(short = 'V')]
    verbose: bool,
    #[clap(short = 'A')]
    extra_file_exts: Vec<String>,
    #[clap(short = 'a', default_value = "css,gif,htm,html,jpeg,jpg,js,png")]
    allowed_file_exts: Vec<String>,
    #[clap(short = 'f')]
    allowed_files: Vec<String>,
    #[clap(short = 'x')]
    excluded_dirs: Vec<String>,
    #[clap(short = 'p')]
    doc_prefix: Option<String>,
    // file targets
    targets: Vec<String>,
}

impl fmt::Display for Options {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let csv_or_none = |val: &[String]| -> String {
            match val.is_empty() {
                true => "none".to_string(),
                false => val.join(","),
            }
        };

        let s = indoc::formatdoc! {r#"
            dohtml:
              recursive: {}
              verbose: {}
              extra file exts: {}
              allowed file exts: {}
              allowed files: {}
              excluded dirs: {}
              doc prefix: {}
        "#,
        self.recursive,
        self.verbose,
        csv_or_none(&self.extra_file_exts),
        csv_or_none(&self.allowed_file_exts),
        csv_or_none(&self.allowed_files),
        csv_or_none(&self.excluded_dirs),
        self.doc_prefix.as_deref().unwrap_or("none"),
        };

        write!(f, "{s}")
    }
}

// Expand a vector of comma-separated strings into a vector of values.
// TODO: replace with internal clap derive parsing?
fn expand_csv(data: &[String]) -> Vec<&str> {
    data.iter().flat_map(|s| s.split(',')).collect()
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let opts = Options::try_parse_from(&[&["dohtml"], args].concat())
        .map_err(|e| Error::Base(format!("invalid args: {e}")))?;

    if opts.targets.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    if opts.verbose {
        write_stderr!("{opts}");
    }

    let mut allowed_file_exts: HashSet<_> =
        expand_csv(&opts.allowed_file_exts).into_iter().collect();
    allowed_file_exts.extend(expand_csv(&opts.extra_file_exts));
    let excluded_dirs: HashSet<PathBuf> = expand_csv(&opts.excluded_dirs)
        .iter()
        .map(PathBuf::from)
        .collect();
    let allowed_files: HashSet<_> = expand_csv(&opts.allowed_files).into_iter().collect();

    // determine if a file is allowed
    let allowed_file = |path: &Path| -> bool {
        match (path.file_name().map(|s| s.to_str()), path.extension().map(|s| s.to_str())) {
            (Some(Some(name)), Some(Some(ext))) => match allowed_files.is_empty() {
                true => allowed_file_exts.contains(ext),
                false => allowed_files.contains(name),
            },
            _ => false,
        }
    };

    // determine if a walkdir entry is allowed
    let is_allowed = |entry: &DirEntry| -> bool {
        let path = entry.path();
        match path.is_dir() {
            true => !excluded_dirs.contains(path),
            false => allowed_file(path),
        }
    };

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();
        let subdir = match d.docdesttree.as_str() {
            "" => "html",
            val => val,
        };
        let pf = d.env.get("PF").expect("$PF undefined");
        let doc_prefix = match opts.doc_prefix.as_ref() {
            None => "",
            Some(s) => s.trim_start_matches('/'),
        };
        let dest = build_from_paths!("/usr/share/doc", pf, subdir, doc_prefix);
        let install = d.install().dest(&dest)?;

        let (dirs, files): (Vec<_>, Vec<_>) =
            opts.targets.iter().map(Path::new).partition(|p| p.is_dir());

        if !dirs.is_empty() {
            if opts.recursive {
                install.recursive(dirs, Some(is_allowed))?;
            } else {
                return Err(Error::Base(format!(
                    "trying to install directory as file: {:?}",
                    dirs[0]
                )));
            }
        }

        let files = files.iter().filter(|f| allowed_file(f));
        install.files(files)?;

        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "dohtml path/to/html/files";
make_builtin!("dohtml", dohtml_builtin, run, LONG_DOC, USAGE, &[("0-6", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::macros::assert_err_re;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::{assert_stderr, BUILD_DATA};

    use super::super::docinto::run as docinto;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dohtml;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dohtml, &[0]);

        BUILD_DATA.with(|d| d.borrow_mut().env.insert("PF".into(), "pkgcraft-0".into()));
        let _file_tree = FileTree::new();

        // non-recursive directory
        fs::create_dir("dir").unwrap();
        let r = dohtml(&["dir"]);
        assert_err_re!(r, format!("^trying to install directory as file: .*$"));
    }

    #[test]
    fn verbose_output() {
        BUILD_DATA.with(|d| d.borrow_mut().env.insert("PF".into(), "pkgcraft-0".into()));
        let _file_tree = FileTree::new();
        fs::File::create("pkgcraft.html").unwrap();

        // defaults
        dohtml(&["-V", "pkgcraft.html"]).unwrap();
        let s = indoc::formatdoc! {r#"
            dohtml:
              recursive: false
              verbose: true
              extra file exts: none
              allowed file exts: css,gif,htm,html,jpeg,jpg,js,png
              allowed files: none
              excluded dirs: none
              doc prefix: none
        "#};
        assert_stderr!(s);

        // extra options
        dohtml(&["-V", "-A", "svg,tiff", "-p", "docs", "pkgcraft.html"]).unwrap();
        let s = indoc::formatdoc! {r#"
            dohtml:
              recursive: false
              verbose: true
              extra file exts: svg,tiff
              allowed file exts: css,gif,htm,html,jpeg,jpg,js,png
              allowed files: none
              excluded dirs: none
              doc prefix: docs
        "#};
        assert_stderr!(s);
    }

    #[test]
    fn creation() {
        BUILD_DATA.with(|d| d.borrow_mut().env.insert("PF".into(), "pkgcraft-0".into()));
        let file_tree = FileTree::new();

        // simple file
        fs::File::create("pkgcraft.html").unwrap();
        dohtml(&["pkgcraft.html"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/html/pkgcraft.html"
        "#
        ));

        // recursive
        fs::create_dir_all("doc/subdir").unwrap();
        fs::File::create("doc/subdir/pkgcraft.html").unwrap();
        dohtml(&["-r", "doc"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/html/doc/subdir/pkgcraft.html"
        "#,
        );

        // recursive using `docinto`
        docinto(&["newdir"]).unwrap();
        dohtml(&["-r", "doc"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/newdir/doc/subdir/pkgcraft.html"
        "#,
        );
    }

    #[test]
    fn options() {
        BUILD_DATA.with(|d| d.borrow_mut().env.insert("PF".into(), "pkgcraft-0".into()));
        let file_tree = FileTree::new();

        fs::create_dir("doc").unwrap();
        fs::File::create("doc/readme.html").unwrap();
        fs::File::create("doc/readme.txt").unwrap();

        // ignored files
        dohtml(&["-r", "doc/."]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/html/readme.html"
        "#,
        );

        // -A: extra allowed file exts
        dohtml(&["-r", "doc/.", "-A", "txt,md"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/html/readme.html"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/html/readme.txt"
        "#,
        );

        // -a: allowed file exts
        dohtml(&["-r", "doc/.", "-a", "txt,md"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/html/readme.txt"
        "#,
        );

        // -f: allowed files
        dohtml(&["-r", "doc/.", "-f", "readme.txt,readme.md"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/html/readme.txt"
        "#,
        );

        // -p: doc prefix
        dohtml(&["-r", "doc/.", "-p", "prefix"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/html/prefix/readme.html"
        "#,
        );

        fs::create_dir("doc/subdir").unwrap();
        fs::File::create("doc/subdir/excluded.html").unwrap();

        // -x: excluded dirs
        dohtml(&["-r", "doc/.", "-x", "doc/subdir,doc/test"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/html/readme.html"
        "#,
        );
    }
}
