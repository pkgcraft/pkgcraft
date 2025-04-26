use camino::Utf8PathBuf;
use itertools::Either;
use scallop::ExecStatus;

use crate::eapi::Feature::ConsistentFileOpts;
use crate::shell::get_build_mut;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "doinitd",
    long_about = "Install init scripts into /etc/init.d/."
)]
struct Command {
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();
    let dest = "/etc/init.d";
    let opts = if build.eapi().has(ConsistentFileOpts) {
        Either::Left(["-m0755"].into_iter())
    } else {
        Either::Right(build.exeopts.iter().map(|s| s.as_str()))
    };
    build
        .install()
        .dest(dest)?
        .file_options(opts)
        .files(&cmd.paths)?;
    Ok(ExecStatus::Success)
}

make_builtin!("doinitd", doinitd_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::EAPIS_OFFICIAL;
    use crate::shell::BuildData;
    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, doinitd, exeopts};
    use super::*;

    cmd_scope_tests!("doinitd path/to/init/file");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(doinitd, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = doinitd(&["nonexistent"]);
        assert_err_re!(r, "^invalid file: nonexistent: No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o100755;
        let custom_mode = 0o100777;

        fs::File::create("pkgcraft").unwrap();
        doinitd(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/etc/init.d/pkgcraft"
            mode = {default_mode}
        "#
        ));

        // verify exeopts are respected depending on EAPI
        for eapi in &*EAPIS_OFFICIAL {
            BuildData::empty(eapi);
            exeopts(&["-m0777"]).unwrap();
            doinitd(&["pkgcraft"]).unwrap();
            let mode = if eapi.has(ConsistentFileOpts) {
                default_mode
            } else {
                custom_mode
            };
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/etc/init.d/pkgcraft"
                mode = {mode}
            "#
            ));
        }
    }
}
