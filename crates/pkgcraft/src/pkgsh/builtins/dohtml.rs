use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use clap::Parser;
use scallop::builtins::ExecStatus;
use scallop::{variables, Error};
use walkdir::DirEntry;

use crate::macros::build_from_paths;
use crate::pkgsh::{get_build_mut, write_stderr};

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
            if val.is_empty() {
                "none".to_string()
            } else {
                val.join(",")
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

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let opts = Options::try_parse_from([&["dohtml"], args].concat())
        .map_err(|e| Error::Base(format!("invalid args: {e}")))?;

    if opts.targets.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    if opts.verbose {
        write_stderr!("{opts}")?;
    }

    // TODO: replace csv expansion with clap arg parsing?
    let mut allowed_file_exts: HashSet<_> = opts
        .allowed_file_exts
        .iter()
        .flat_map(|s| s.split(','))
        .collect();
    allowed_file_exts.extend(opts.extra_file_exts.iter().flat_map(|s| s.split(',')));
    let excluded_dirs: HashSet<_> = opts
        .excluded_dirs
        .iter()
        .flat_map(|s| s.split(','))
        .map(PathBuf::from)
        .collect();
    let allowed_files: HashSet<_> = opts
        .allowed_files
        .iter()
        .flat_map(|s| s.split(','))
        .collect();

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

    let build = get_build_mut();
    let subdir = match build.docdesttree.as_str() {
        "" => "html",
        val => val,
    };
    let doc_prefix = match opts.doc_prefix.as_ref() {
        None => "",
        Some(s) => s.trim_start_matches('/'),
    };
    let dest = build_from_paths!("/usr/share/doc", variables::required("PF")?, subdir, doc_prefix);
    let install = build.install().dest(dest)?;

    let (dirs, files): (Vec<_>, Vec<_>) =
        opts.targets.iter().map(Path::new).partition(|p| p.is_dir());

    if !dirs.is_empty() {
        if opts.recursive {
            install.recursive(dirs, Some(is_allowed))?;
        } else {
            return Err(Error::Base(format!("trying to install directory as file: {:?}", dirs[0])));
        }
    }

    let files = files.iter().filter(|f| allowed_file(f));
    install.files(files)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "dohtml path/to/html/files";
make_builtin!("dohtml", dohtml_builtin, run, LONG_DOC, USAGE, &[("0..7", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use scallop::variables::bind;

    use crate::macros::assert_err_re;
    use crate::pkgsh::assert_stderr;
    use crate::pkgsh::test::FileTree;

    use super::super::docinto::run as docinto;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dohtml;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dohtml, &[0]);

        bind("PF", "pkg-1", None, None).unwrap();
        let _file_tree = FileTree::new();

        // non-recursive directory
        fs::create_dir("dir").unwrap();
        let r = dohtml(&["dir"]);
        assert_err_re!(r, "^trying to install directory as file: .*$");
    }

    #[test]
    fn verbose_output() {
        bind("PF", "pkg-1", None, None).unwrap();
        let _file_tree = FileTree::new();
        fs::File::create("index.html").unwrap();

        // defaults
        dohtml(&["-V", "index.html"]).unwrap();
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
        dohtml(&["-V", "-A", "svg,tiff", "-p", "docs", "index.html"]).unwrap();
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
        bind("PF", "pkg-1", None, None).unwrap();
        let file_tree = FileTree::new();

        // simple file
        fs::File::create("index.html").unwrap();
        dohtml(&["index.html"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/index.html"
        "#,
        );

        // recursive
        fs::create_dir_all("doc/subdir").unwrap();
        fs::File::create("doc/subdir/index.html").unwrap();
        dohtml(&["-r", "doc"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/doc/subdir/index.html"
        "#,
        );

        // recursive using `docinto`
        docinto(&["newdir"]).unwrap();
        dohtml(&["-r", "doc"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/newdir/doc/subdir/index.html"
        "#,
        );
    }

    #[test]
    fn options() {
        bind("PF", "pkg-1", None, None).unwrap();
        let file_tree = FileTree::new();

        fs::create_dir("doc").unwrap();
        fs::File::create("doc/readme.html").unwrap();
        fs::File::create("doc/readme.txt").unwrap();

        // ignored files
        dohtml(&["-r", "doc/."]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/readme.html"
        "#,
        );

        // -A: extra allowed file exts
        dohtml(&["-r", "doc/.", "-A", "txt,md"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/readme.html"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/readme.txt"
        "#,
        );

        // -a: allowed file exts
        dohtml(&["-r", "doc/.", "-a", "txt,md"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/readme.txt"
        "#,
        );

        // -f: allowed files
        dohtml(&["-r", "doc/.", "-f", "readme.txt,readme.md"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/readme.txt"
        "#,
        );

        // -p: doc prefix
        dohtml(&["-r", "doc/.", "-p", "prefix"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/prefix/readme.html"
        "#,
        );

        fs::create_dir("doc/subdir").unwrap();
        fs::File::create("doc/subdir/excluded.html").unwrap();

        // -x: excluded dirs
        dohtml(&["-r", "doc/.", "-x", "doc/subdir,doc/test"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/html/readme.html"
        "#,
        );
    }
}
