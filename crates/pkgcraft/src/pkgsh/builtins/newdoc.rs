use scallop::builtins::ExecStatus;

use super::_new::new;
use super::dodoc::run as dodoc;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed documentation files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, dodoc)
}

const USAGE: &str = "newdoc path/to/doc/file new_filename";
make_builtin!("newdoc", newdoc_builtin, run, LONG_DOC, USAGE, &[("..", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::config::Config;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::{write_stdin, BuildData};
    use crate::repo::PkgRepository;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as newdoc;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(newdoc, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();
        let (_, cpv) = t.create_ebuild("cat/pkg-1", &[]).unwrap();
        let pkg = repo.iter_restrict(&cpv).next().unwrap();
        BuildData::from_pkg(&pkg);
        let file_tree = FileTree::new();

        fs::File::create("file").unwrap();
        newdoc(&["file", "newfile"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/newfile"
        "#,
        );

        // re-run using data from stdin
        write_stdin!("stdin");
        newdoc(&["-", "newfile"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/newfile"
            data = "stdin"
        "#,
        );
    }
}
