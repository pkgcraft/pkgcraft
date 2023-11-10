use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "\
Sets the options for directory creation via `dodir` and similar commands.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    get_build_mut().diropts = args.iter().map(|s| s.to_string()).collect();

    Ok(ExecStatus::Success)
}

const USAGE: &str = "diropts -m0750";
make_builtin!("diropts", diropts_builtin, run, LONG_DOC, USAGE, BUILTIN);

#[cfg(test)]
mod tests {
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_args, builtin_scope_tests, dodir};
    use super::BUILTIN as diropts;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(diropts, &[0]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        // change mode and re-run dodir()
        diropts(&["-m0777"]).unwrap();
        dodir(&["dir"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/dir"
            mode = 0o40777
        "#,
        );
    }
}
