use camino::Utf8PathBuf;
use itertools::Either;
use scallop::ExecStatus;

use crate::eapi::Feature::ConsistentFileOpts;
use crate::shell::get_build_mut;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "doconfd",
    disable_help_flag = true,
    long_about = "Install config files into /etc/conf.d/."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = true, allow_hyphen_values = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();
    let dest = "/etc/conf.d";
    let opts = if build.eapi().has(ConsistentFileOpts) {
        Either::Left(["-m0644"].into_iter())
    } else {
        Either::Right(build.insopts.iter().map(|s| s.as_str()))
    };
    build
        .install()
        .dest(dest)?
        .file_options(opts)
        .files(&cmd.paths)?;
    Ok(ExecStatus::Success)
}

make_builtin!("doconfd", doconfd_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::EAPIS_OFFICIAL;
    use crate::shell::BuildData;
    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{
        assert_invalid_cmd, cmd_scope_tests,
        functions::{doconfd, insopts},
    };
    use super::*;

    cmd_scope_tests!("doconfd path/to/conf/file");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(doconfd, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = doconfd(&["nonexistent"]);
        assert_err_re!(r, "^invalid file: nonexistent: No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o100644;
        let custom_mode = 0o100755;

        fs::File::create("pkgcraft").unwrap();
        doconfd(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/etc/conf.d/pkgcraft"
            mode = {default_mode}
        "#
        ));

        // verify insopts are respected depending on EAPI
        for eapi in &*EAPIS_OFFICIAL {
            BuildData::empty(eapi);
            insopts(&["-m0755"]).unwrap();
            doconfd(&["pkgcraft"]).unwrap();
            let mode = if eapi.has(ConsistentFileOpts) {
                default_mode
            } else {
                custom_mode
            };
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/etc/conf.d/pkgcraft"
                mode = {mode}
            "#
            ));
        }
    }
}
