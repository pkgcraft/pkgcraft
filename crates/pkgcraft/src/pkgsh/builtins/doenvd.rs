use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::eapi::Feature;
use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "Install environment files into /etc/env.d/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let d = d.borrow();
        let dest = "/etc/env.d";
        let opts: Vec<_> = match d.eapi.has(Feature::ConsistentFileOpts) {
            true => vec!["-m0644"],
            false => d.insopts.iter().map(|s| s.as_str()).collect(),
        };
        let install = d.install().dest(dest)?.file_options(opts);
        install.files(args)?;
        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "doenvd path/to/env/file";
make_builtin!("doenvd", doenvd_builtin, run, LONG_DOC, USAGE, &[("..", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::BUILD_DATA;

    use super::super::insopts::run as insopts;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as doenvd;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(doenvd, &[0]);
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
        for eapi in EAPIS_OFFICIAL.iter() {
            BUILD_DATA.with(|d| d.borrow_mut().eapi = eapi);
            insopts(&["-m0755"]).unwrap();
            doenvd(&["pkgcraft"]).unwrap();
            let mode = match eapi.has(Feature::ConsistentFileOpts) {
                true => default_mode,
                false => custom_mode,
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
