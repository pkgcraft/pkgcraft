use itertools::Either;
use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::eapi::Feature::ConsistentFileOpts;
use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcInstall;

use super::make_builtin;

const LONG_DOC: &str = "Install init scripts into /etc/init.d/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let build = get_build_mut();
    let dest = "/etc/init.d";
    let opts = if build.eapi().has(ConsistentFileOpts) {
        Either::Left(["-m0755"].into_iter())
    } else {
        Either::Right(build.exeopts.iter().map(|s| s.as_str()))
    };
    let install = build.install().dest(dest)?.file_options(opts);
    install.files(args)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "doinitd path/to/init/file";
make_builtin!("doinitd", doinitd_builtin, run, LONG_DOC, USAGE, [("..", [SrcInstall])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::EAPIS_OFFICIAL;
    use crate::macros::assert_err_re;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;

    use super::super::exeopts::run as exeopts;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as doinitd;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(doinitd, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = doinitd(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
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
        for eapi in EAPIS_OFFICIAL.iter() {
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
