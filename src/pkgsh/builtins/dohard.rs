use std::fs::hard_link;

use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "Create hard links.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let (source, target) = match args.len() {
        2 => Ok((args[0], args[1])),
        n => Err(Error::Base(format!("requires 2 args, got {n}"))),
    }?;

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let install = d.borrow().install();
        install.link(|p, q| hard_link(p, q), source, target)?;
        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "dohard path/to/source /path/to/target";
make_builtin!("dohard", dohard_builtin, run, LONG_DOC, USAGE, &[("0-3", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::MetadataExt;

    use crate::pkgsh::test::FileTree;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dohard;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dohard, &[0, 1, 3]);
    }

    #[test]
    fn linking() {
        let file_tree = FileTree::new();
        fs::File::create("source").unwrap();

        dohard(&["source", "target"]).unwrap();
        let source_meta = fs::metadata("source").unwrap();
        let target_meta = fs::metadata(file_tree.install_dir.join("target")).unwrap();
        // hard link inodes match
        assert_eq!(source_meta.ino(), target_meta.ino());
        assert_eq!(target_meta.nlink(), 2);
    }
}
