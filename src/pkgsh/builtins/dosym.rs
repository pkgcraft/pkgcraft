use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::eapi::Feature;
use crate::pkgsh::BUILD_DATA;
use crate::utils::relpath;

use super::make_builtin;

const LONG_DOC: &str = "Create symbolic links.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (source, target, target_str) = match args.len() {
            3 if args[0] == "-r" && eapi.has(Feature::DosymRelative) => {
                let (source, target) = (Path::new(args[1]), Path::new(args[2]));
                if !source.is_absolute() {
                    return Err(Error::Base(format!(
                        "absolute source required with '-r': {source:?}",
                    )));
                }
                let mut parent = PathBuf::from("/");
                if let Some(p) = target.parent() {
                    parent.push(p)
                }
                match relpath(source, &parent) {
                    Some(source) => Ok((source, target, args[2])),
                    None => {
                        Err(Error::Base(format!("invalid relative path: {source:?} -> {target:?}")))
                    }
                }
            }
            2 => Ok((PathBuf::from(args[0]), Path::new(args[1]), args[1])),
            n => Err(Error::Base(format!("requires 2 args, got {n}"))),
        }?;

        // check for unsupported dir target arg -- https://bugs.gentoo.org/379899
        if target_str.ends_with('/') || (target.is_dir() && !target.is_symlink()) {
            return Err(Error::Base(format!("missing filename target: {target:?}")));
        }

        let install = d.borrow().install();
        install.link(|p, q| symlink(p, q), source, target)?;

        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "dosym path/to/source /path/to/target";
make_builtin!("dosym", dosym_builtin, run, LONG_DOC, USAGE, &[("..", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::BUILD_DATA;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dosym;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dosym, &[0, 1, 4]);

        BUILD_DATA.with(|d| {
            for eapi in EAPIS_OFFICIAL
                .iter()
                .filter(|e| !e.has(Feature::DosymRelative))
            {
                d.borrow_mut().eapi = eapi;
                assert_invalid_args(dosym, &[3]);
            }
        });
    }

    #[test]
    fn errors() {
        let _file_tree = FileTree::new();

        // dir targets aren't supported
        let r = dosym(&["source", "target/"]);
        assert_err_re!(r, "^missing filename target: .*$");

        fs::create_dir("target").unwrap();
        let r = dosym(&["source", "target"]);
        assert_err_re!(r, "^missing filename target: .*$");

        // relative source with `dosym -r`
        let r = dosym(&["-r", "source", "target"]);
        assert_err_re!(r, "^absolute source required .*$");
    }

    #[test]
    fn linking() {
        let file_tree = FileTree::new();

        dosym(&["/usr/bin/source", "target"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/target"
            link = "/usr/bin/source"
        "#
        ));

        dosym(&["-r", "/usr/bin/source", "/usr/bin/target"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/bin/target"
            link = "source"
        "#
        ));
    }
}
