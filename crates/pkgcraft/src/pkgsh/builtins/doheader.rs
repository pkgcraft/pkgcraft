use std::path::Path;

use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::eapi::Feature;
use crate::files::NO_WALKDIR_FILTER;
use crate::pkgsh::get_build_mut;
use crate::pkgsh::phase::PhaseKind::SrcInstall;

use super::{make_builtin, Scopes::Phase};

const LONG_DOC: &str = "Install header files into /usr/include/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let (recursive, args) = match args.first() {
        Some(&"-r") => Ok((true, &args[1..])),
        Some(_) => Ok((false, args)),
        None => Err(Error::Base("requires 1 or more args, got 0".into())),
    }?;

    let build = get_build_mut();
    let dest = "/usr/include";
    let opts: Vec<_> = if build.eapi().has(Feature::ConsistentFileOpts) {
        vec!["-m0644"]
    } else {
        build.insopts.iter().map(|s| s.as_str()).collect()
    };
    let install = build.install().dest(dest)?.file_options(opts);

    let (dirs, files): (Vec<_>, Vec<_>) = args.iter().map(Path::new).partition(|p| p.is_dir());

    if !dirs.is_empty() {
        if recursive {
            install.recursive(dirs, NO_WALKDIR_FILTER)?;
        } else {
            return Err(Error::Base(format!("trying to install directory as file: {:?}", dirs[0])));
        }
    }

    install.files(files)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "doheader path/to/header.h";
make_builtin!("doheader", doheader_builtin, run, LONG_DOC, USAGE, &[("5..", &[Phase(SrcInstall)])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::BuildData;

    use super::super::insopts::run as insopts;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as doheader;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(doheader, &[0]);

        let _file_tree = FileTree::new();

        // non-recursive directory
        fs::create_dir("dir").unwrap();
        let r = doheader(&["dir"]);
        assert_err_re!(r, "^trying to install directory as file: .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o100644;
        let custom_mode = 0o100755;

        // simple file
        fs::File::create("pkgcraft.h").unwrap();
        doheader(&["pkgcraft.h"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/include/pkgcraft.h"
            mode = {default_mode}
        "#
        ));

        // recursive
        fs::create_dir_all("pkgcraft").unwrap();
        fs::File::create("pkgcraft/pkgcraft.h").unwrap();
        for eapi in EAPIS_OFFICIAL.iter() {
            BuildData::empty(eapi);
            insopts(&["-m0755"]).unwrap();
            doheader(&["-r", "pkgcraft"]).unwrap();
            let mode = if eapi.has(Feature::ConsistentFileOpts) {
                default_mode
            } else {
                custom_mode
            };
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/usr/include/pkgcraft/pkgcraft.h"
                mode = {mode}
            "#
            ));
        }

        // handling for paths ending in '/.'
        doheader(&["-r", "pkgcraft/."]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/include/pkgcraft.h"
        "#,
        );
    }
}
