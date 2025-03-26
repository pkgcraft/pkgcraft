use camino::Utf8PathBuf;
use itertools::Either;
use scallop::ExecStatus;

use crate::eapi::Feature::ConsistentFileOpts;
use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "doenvd",
    long_about = "Install environment files into /etc/env.d/."
)]
struct Command {
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();
    let dest = "/etc/env.d";
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

const USAGE: &str = "doenvd path/to/env/file";
make_builtin!("doenvd", doenvd_builtin, true);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::EAPIS_OFFICIAL;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, doenvd, insopts};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(doenvd, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = doenvd(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o100644;
        let custom_mode = 0o100755;

        fs::File::create("pkgcraft").unwrap();
        doenvd(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/etc/env.d/pkgcraft"
            mode = {default_mode}
        "#
        ));

        // verify insopts are respected depending on EAPI
        for eapi in &*EAPIS_OFFICIAL {
            BuildData::empty(eapi);
            insopts(&["-m0755"]).unwrap();
            doenvd(&["pkgcraft"]).unwrap();
            let mode = if eapi.has(ConsistentFileOpts) {
                default_mode
            } else {
                custom_mode
            };
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/etc/env.d/pkgcraft"
                mode = {mode}
            "#
            ));
        }
    }
}
