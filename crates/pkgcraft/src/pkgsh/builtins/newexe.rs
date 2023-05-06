use scallop::builtins::ExecStatus;

use super::_new::new;
use super::doexe::run as doexe;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed environment files into /etc/env.d/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, doexe)
}

const USAGE: &str = "newexe path/to/executable new_filename";
make_builtin!("newexe", newexe_builtin, run, LONG_DOC, USAGE, &[("..", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::config::Config;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::{write_stdin, BuildData};

    use super::super::exeinto::run as exeinto;
    use super::super::exeopts::run as exeopts;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as newexe;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(newexe, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();
        let (_, cpv) = t.create_ebuild("cat/pkg-1", []).unwrap();
        BuildData::update(&cpv, &repo);

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
