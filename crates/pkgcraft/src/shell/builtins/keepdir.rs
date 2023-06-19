use std::fs::File;

use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkg::Package;
use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcInstall;

use super::{make_builtin, Scopes::Phase};

const LONG_DOC: &str = "Install directories.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let build = get_build_mut();
    let install = build.install();

    // use custom file name including pkg info
    let pkg = build.pkg()?;
    let (cat, pkg, slot) = (pkg.cpv().category(), pkg.cpv().package(), pkg.slot());
    let file_name = format!(".keep_{cat}_{pkg}_{slot}");

    // create dirs
    install.dirs(args)?;

    // create stub files
    for path in args {
        let keep = install.prefix(path).join(&file_name);
        File::create(&keep)
            .map_err(|e| Error::Base(format!("failed creating keep file: {keep:?}: {e}")))?;
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "keepdir path/to/kept/dir";
make_builtin!("keepdir", keepdir_builtin, run, LONG_DOC, USAGE, &[("..", &[Phase(SrcInstall)])]);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::pkg::BuildablePackage;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as keepdir;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(keepdir, &[0]);
    }

    #[test]
    fn creation() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let default_mode = 0o100644;

        for dirs in [
            vec!["dir"],
            vec!["path/to/dir"],
            vec!["/etc"],
            vec!["/usr/bin"],
            vec!["dir", "/usr/bin"],
        ] {
            let args = dirs.join(" ");
            let data = indoc::formatdoc! {r#"
                EAPI=8
                DESCRIPTION="testing keepdir"
                SLOT=0
                src_install() {{
                    keepdir {args}
                }}
            "#};
            let raw_pkg = t.create_ebuild_raw("cat/pkg-1", &data).unwrap();
            let pkg = raw_pkg.into_pkg().unwrap();
            BuildData::from_pkg(&pkg);
            let file_tree = FileTree::new();
            pkg.build().unwrap();

            let mut files = vec![];
            for dir in dirs {
                let path = dir.trim_start_matches('/');
                files.push(format!(
                    r#"
                    [[files]]
                    path = "/{path}/.keep_cat_pkg_0"
                    mode = {default_mode}
                    data = ""
                "#
                ));
            }
            file_tree.assert(files.join("\n"));
        }
    }
}
