use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::eapi::Feature;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install environment files into /etc/env.d/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();
        let dest = "/etc/env.d";
        let opts: Vec<&str> = match d.eapi.has(Feature::ConsistentFileOpts) {
            true => vec!["-m0644"],
            false => d.insopts.iter().map(|s| s.as_str()).collect(),
        };
        let install = d.install().dest(&dest)?.file_options(opts.iter().copied());
        install.files(args)?;
        Ok(ExecStatus::Success)
    })
}

make_builtin!("doenvd", doenvd_builtin, run, LONG_DOC, "doenvd path/to/env/file");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &["src_install"])]));

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::assert_invalid_args;
    use super::super::insopts::run as insopts;
    use super::run as doenvd;
    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::BUILD_DATA;

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
        for eapi in EAPIS_OFFICIAL.values() {
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
