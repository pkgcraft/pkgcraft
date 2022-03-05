use std::path::Path;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install GNU info files into /usr/share/info/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let dest = "/usr/share/info";
        let opts = ["-m0644"];
        let install = d.borrow().install().dest(&dest)?.ins_options(opts);

        let files = args
            .iter()
            .map(Path::new)
            .filter_map(|f| f.file_name().map(|name| (f, name)));
        install.files(files)?;

        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "doinfo",
            func: run,
            help: LONG_DOC,
            usage: "doinfo path/to/info/file",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::fs;

    use rusty_fork::rusty_fork_test;

    use super::super::assert_invalid_args;
    use super::run as doinfo;
    use crate::pkgsh::test::FileTree;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(doinfo, &[0]);
        }

        #[test]
        fn creation() {
            let file_tree = FileTree::new();
            let default_mode = 0o100644;

            fs::File::create("pkgcraft").unwrap();
            doinfo(&["pkgcraft"]).unwrap();
            file_tree.assert(format!(r#"
                [[files]]
                path = "/usr/share/info/pkgcraft"
                mode = {default_mode}
            "#));
        }
    }
}
