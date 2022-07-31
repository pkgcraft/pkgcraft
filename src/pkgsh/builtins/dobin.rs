use nix::unistd::geteuid;
use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::macros::build_from_paths;
use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "Install executables into DESTTREE/bin.";

pub(super) fn install_bin(args: &[&str], dest: &str) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let dest = build_from_paths!(&d.borrow().desttree, dest);
        let opts: &[&str] = match geteuid().is_root() {
            true => &["-m0755", "-o", "root", "-g", "root"],
            false => &["-m0755"],
        };
        let install = d
            .borrow()
            .install()
            .dest(&dest)?
            .file_options(opts.iter().copied());

        install.files(args)?;
        Ok(ExecStatus::Success)
    })
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    install_bin(args, "bin")
}

const USAGE: &str = "dobin path/to/executable";
make_builtin!("dobin", dobin_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::pkgsh::test::FileTree;

    use super::super::exeopts::run as exeopts;
    use super::super::into::run as into;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dobin;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dobin, &[0]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o100755;

        fs::File::create("pkgcraft").unwrap();
        dobin(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/bin/pkgcraft"
            mode = {default_mode}
        "#
        ));

        // custom install dir with libopts ignored
        into(&["/"]).unwrap();
        exeopts(&["-m0777"]).unwrap();
        dobin(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/bin/pkgcraft"
            mode = {default_mode}
        "#
        ));
    }
}
