use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::doexe::run as doexe;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install renamed environment files into /etc/env.d/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, doexe)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "newexe",
            func: run,
            help: LONG_DOC,
            usage: "newexe path/to/executable new_filename",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::super::assert_invalid_args;
    use super::super::exeinto::run as exeinto;
    use super::super::exeopts::run as exeopts;
    use super::run as newexe;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::write_stdin;

    #[test]
    fn invalid_args() {
        assert_invalid_args(newexe, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o100755;
        let custom_mode = 0o100777;

        fs::File::create("bin").unwrap();
        newexe(&["bin", "pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/pkgcraft"
            mode = {default_mode}
        "#
        ));

        // custom mode and install dir using data from stdin
        write_stdin!("pkgcraft");
        exeinto(&["/opt/bin"]).unwrap();
        exeopts(&["-m0777"]).unwrap();
        newexe(&["-", "pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/opt/bin/pkgcraft"
            mode = {custom_mode}
            data = "pkgcraft"
        "#
        ));
    }
}
