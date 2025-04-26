use camino::Utf8PathBuf;
use itertools::Either;
use scallop::{Error, ExecStatus};

use crate::eapi::Feature::ConsistentFileOpts;
use crate::files::NO_WALKDIR_FILTER;
use crate::shell::get_build_mut;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "doheader",
    disable_help_flag = true,
    long_about = "Install header files into /usr/include."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(short = 'r')]
    recursive: bool,

    #[arg(required = true, allow_hyphen_values = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();
    let dest = "/usr/include";
    let opts = if build.eapi().has(ConsistentFileOpts) {
        Either::Left(["-m0644"].into_iter())
    } else {
        Either::Right(build.insopts.iter().map(|s| s.as_str()))
    };
    let install = build.install().dest(dest)?.file_options(opts);

    let (dirs, files): (Vec<_>, Vec<_>) = cmd.paths.iter().partition(|p| p.is_dir());

    if let Some(dir) = dirs.first() {
        if cmd.recursive {
            install.recursive(dirs, NO_WALKDIR_FILTER)?;
        } else {
            return Err(Error::Base(format!("installing directory without -r: {dir}")));
        }
    }

    if !files.is_empty() {
        install.files(files)?;
    }

    Ok(ExecStatus::Success)
}

make_builtin!("doheader", doheader_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::EAPIS_OFFICIAL;
    use crate::shell::BuildData;
    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, doheader, insopts};
    use super::*;

    cmd_scope_tests!("doheader path/to/header.h");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(doheader, &[0]);

        // missing args
        assert!(doheader(&["-r"]).is_err());

        let _file_tree = FileTree::new();

        // non-recursive directory
        fs::create_dir("dir").unwrap();
        let r = doheader(&["dir"]);
        assert_err_re!(r, "^installing directory without -r: dir$");

        // nonexistent
        let r = doheader(&["nonexistent"]);
        assert_err_re!(r, "^invalid file: nonexistent: No such file or directory .*$");
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
