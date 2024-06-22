use scallop::ExecStatus;

use super::_new::new;
use super::dodoc;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed documentation files.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, dodoc)
}

const USAGE: &str = "newdoc path/to/doc/file new_filename";
make_builtin!("newdoc", newdoc_builtin);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::config::Config;
    use crate::shell::test::FileTree;
    use crate::shell::{write_stdin, BuildData};

    use super::super::{assert_invalid_args, cmd_scope_tests, newdoc};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(newdoc, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let mut config = Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();
        let pkg = repo.create_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_pkg(&pkg);
        let file_tree = FileTree::new();

        fs::File::create("file").unwrap();
        newdoc(&["file", "newfile"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/newfile"
            mode = 0o100644
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
            mode = 0o100644
        "#,
        );
    }
}
