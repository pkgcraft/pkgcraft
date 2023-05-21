use std::fs::File;

use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkg::Package;
use crate::pkgsh::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Install directories.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let build = get_build_mut();
    let install = build.install();
    let pkg = build.pkg()?;

    // create dirs
    install.dirs(args)?;

    // create stub files
    for path in args {
        let (cat, pkg, slot) = (pkg.cpv().category(), pkg.cpv().package(), pkg.slot());
        let file_name = format!(".keep_{cat}_{pkg}_{slot}");
        let keep = install.prefix(path).join(file_name);
        File::create(&keep)
            .map_err(|e| Error::Base(format!("failed creating keep file: {keep:?}: {e}")))?;
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "keepdir path/to/kept/dir";
make_builtin!("keepdir", keepdir_builtin, run, LONG_DOC, USAGE, &[("..", &["src_install"])]);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::pkg::BuildablePackage;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::BuildData;
    use crate::repo::PkgRepository;

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
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();
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
            let (_, cpv) = t.create_ebuild_raw("cat/pkg-1", &data).unwrap();
            let pkg = repo.iter_restrict(&cpv).next().unwrap();
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
