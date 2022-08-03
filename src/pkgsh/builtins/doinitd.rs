use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::eapi::Feature;
use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "Install init scripts into /etc/init.d/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();
        let dest = "/etc/init.d";
        let opts = match d.eapi.has(Feature::ConsistentFileOpts) {
            true => vec!["-m0755"],
            false => d.exeopts.iter().map(|s| s.as_str()).collect(),
        };
        let install = d.install().dest(&dest)?.file_options(opts);
        install.files(args)?;
        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "doinitd path/to/init/file";
make_builtin!("doinitd", doinitd_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::BUILD_DATA;

    use super::super::exeopts::run as exeopts;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as doinitd;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(doinitd, &[0]);
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
        for eapi in EAPIS_OFFICIAL.values() {
            BUILD_DATA.with(|d| d.borrow_mut().eapi = eapi);
            exeopts(&["-m0777"]).unwrap();
            doinitd(&["pkgcraft"]).unwrap();
            let mode = match eapi.has(Feature::ConsistentFileOpts) {
                true => default_mode,
                false => custom_mode,
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
