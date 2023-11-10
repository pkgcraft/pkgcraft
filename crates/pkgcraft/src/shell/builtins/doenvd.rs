use itertools::Either;
use scallop::{Error, ExecStatus};

use crate::eapi::Feature::ConsistentFileOpts;
use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Install environment files into /etc/env.d/.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let build = get_build_mut();
    let dest = "/etc/env.d";
    let opts = if build.eapi().has(ConsistentFileOpts) {
        Either::Left(["-m0644"].into_iter())
    } else {
        Either::Right(build.insopts.iter().map(|s| s.as_str()))
    };
    let install = build.install().dest(dest)?.file_options(opts);
    install.files(args)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "doenvd path/to/env/file";
make_builtin!("doenvd", doenvd_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::EAPIS_OFFICIAL;
    use crate::macros::assert_err_re;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, builtin_scope_tests, insopts};
    use super::BUILTIN as doenvd;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(doenvd, &[0]);

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
