use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Install GNU info files into /usr/share/info/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let dest = "/usr/share/info";
    let opts = ["-m0644"];
    let install = get_build_mut().install().dest(dest)?.file_options(opts);
    install.files(args)?;
    Ok(ExecStatus::Success)
}

const USAGE: &str = "doinfo path/to/info/file";
make_builtin!("doinfo", doinfo_builtin, run, LONG_DOC, USAGE, BUILTIN);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::macros::assert_err_re;
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as doinfo;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(doinfo, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = doinfo(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("pkgcraft").unwrap();
        doinfo(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/info/pkgcraft"
            mode = 0o100644
        "#,
        );
    }
}
