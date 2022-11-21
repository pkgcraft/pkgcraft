use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::macros::build_from_paths;
use crate::pkgsh::utils::get_libdir;
use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "Install libraries.";

pub(super) fn install_lib(args: &[&str], opts: Option<Vec<&str>>) -> scallop::Result<ExecStatus> {
    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let d = d.borrow();
        let libdir = get_libdir(Some("lib")).unwrap();
        let dest = build_from_paths!(&d.desttree, &libdir);
        let opts: Vec<&str> = match opts {
            Some(v) => v,
            None => d.libopts.iter().map(|s| s.as_str()).collect(),
        };
        let install = d.install().dest(&dest)?.file_options(opts);
        install.files(args)?;
        Ok(ExecStatus::Success)
    })
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    install_lib(args, None)
}

const USAGE: &str = "dolib path/to/lib";
make_builtin!("dolib", dolib_builtin, run, LONG_DOC, USAGE, &[("0-6", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::BuildData;

    use super::super::into::run as into;
    use super::super::libopts::run as libopts;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dolib;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dolib, &[0]);
    }

    #[test]
    fn creation() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, _) = config.temp_repo("test", 0).unwrap();
        let (_, cpv) = t.create_ebuild("cat/pkg-1", []).unwrap();
        BuildData::update(&cpv, "test");

        let file_tree = FileTree::new();
        let default_mode = 0o100644;
        let custom_mode = 0o100755;

        fs::File::create("pkgcraft").unwrap();
        dolib(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft"
            mode = {default_mode}
        "#
        ));

        // custom mode and install dir
        into(&["/"]).unwrap();
        libopts(&["-m0755"]).unwrap();
        dolib(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/lib/pkgcraft"
            mode = {custom_mode}
        "#
        ));
    }
}
