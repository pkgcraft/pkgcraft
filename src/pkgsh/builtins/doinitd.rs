use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install init scripts into /etc/init.d/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();
        let dest = "/etc/init.d";
        let opts: Vec<&str> = match d.eapi.has("consistent_file_opts") {
            true => vec!["-m0755"],
            false => d.exeopts.iter().map(|s| s.as_str()).collect(),
        };
        let install = d.install().dest(&dest)?.ins_options(opts.iter().copied());
        install.files(args)?;
        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "doinitd",
            func: run,
            help: LONG_DOC,
            usage: "doinitd path/to/init/file",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::fs;

    use rusty_fork::rusty_fork_test;

    use super::super::assert_invalid_args;
    use super::super::exeopts::run as exeopts;
    use super::run as doinitd;
    use crate::eapi::OFFICIAL_EAPIS;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::BUILD_DATA;

    rusty_fork_test! {
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
            file_tree.assert(format!(r#"
                [[files]]
                path = "/etc/init.d/pkgcraft"
                mode = {default_mode}
            "#));

            // verify exeopts are respected depending on EAPI
            for eapi in OFFICIAL_EAPIS.values() {
                BUILD_DATA.with(|d| d.borrow_mut().eapi = eapi);
                exeopts(&["-m0777"]).unwrap();
                doinitd(&["pkgcraft"]).unwrap();
                let mode = match eapi.has("consistent_file_opts") {
                    true => default_mode,
                    false => custom_mode,
                };
                file_tree.assert(format!(r#"
                    [[files]]
                    path = "/etc/init.d/pkgcraft"
                    mode = {mode}
                "#));
            }
        }
    }
}
