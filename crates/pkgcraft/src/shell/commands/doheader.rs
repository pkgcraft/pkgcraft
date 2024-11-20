use std::path::Path;

use itertools::Either;
use scallop::{Error, ExecStatus};

use crate::eapi::Feature::ConsistentFileOpts;
use crate::files::NO_WALKDIR_FILTER;
use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Install header files into /usr/include/.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let (recursive, args) = match args {
        ["-r", args @ ..] => (true, args),
        _ => (false, args),
    };

    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".to_string()));
    }

    let build = get_build_mut();
    let dest = "/usr/include";
    let opts = if build.eapi().has(ConsistentFileOpts) {
        Either::Left(["-m0644"].into_iter())
    } else {
        Either::Right(build.insopts.iter().map(|s| s.as_str()))
    };
    let install = build.install().dest(dest)?.file_options(opts);

    let (dirs, files): (Vec<_>, Vec<_>) = args.iter().map(Path::new).partition(|p| p.is_dir());

    if !dirs.is_empty() {
        if recursive {
            install.recursive(dirs, NO_WALKDIR_FILTER)?;
        } else {
            let dir = dirs[0].to_string_lossy();
            return Err(Error::Base(format!("installing directory without -r: {dir}")));
        }
    }

    install.files(files)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "doheader path/to/header.h";
make_builtin!("doheader", doheader_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::EAPIS_OFFICIAL;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_args, cmd_scope_tests, doheader, insopts};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(doheader, &[0]);

        // missing args
        let r = doheader(&["-r"]);
        assert_err_re!(r, "^requires 1 or more args, got 0");

        let _file_tree = FileTree::new();

        // non-recursive directory
        fs::create_dir("dir").unwrap();
        let r = doheader(&["dir"]);
        assert_err_re!(r, "^installing directory without -r: dir$");

        // nonexistent
        let r = doheader(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
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
        for eapi in &*EAPIS_OFFICIAL {
            BuildData::empty(eapi);
            insopts(&["-m0755"]).unwrap();
            doheader(&["-r", "pkgcraft"]).unwrap();
            let mode = if eapi.has(ConsistentFileOpts) {
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
