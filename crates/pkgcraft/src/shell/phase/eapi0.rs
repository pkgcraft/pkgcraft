use std::path::Path;

use is_executable::IsExecutable;
use scallop::builtins::ExecStatus;

use crate::eapi::Feature;
use crate::shell::builtins::{econf::run as econf, emake::run as emake, unpack::run as unpack};
use crate::shell::utils::makefile_exists;
use crate::shell::{write_stderr, BuildData};

pub(crate) fn pkg_nofetch(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    // TODO: only output URLs for missing distfiles
    if !build.distfiles.is_empty() {
        let pkg = build.pkg()?;
        write_stderr!("The following files must be manually downloaded for {pkg}:\n")?;
        for url in &build.distfiles {
            write_stderr!("{url}\n")?;
        }
    }

    Ok(ExecStatus::Success)
}

pub(crate) fn src_unpack(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    let args: Vec<_> = build.distfiles.iter().map(|s| s.as_str()).collect();
    if args.is_empty() {
        Ok(ExecStatus::Success)
    } else {
        unpack(&args)
    }
}

pub(crate) fn src_compile(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    if Path::new("./configure").is_executable() {
        econf(&[])?;
    }
    if makefile_exists() {
        emake(&[])?;
    }
    Ok(ExecStatus::Success)
}

pub(crate) fn src_test(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    for target in ["check", "test"] {
        if emake(&[target, "-n"]).is_ok() {
            if build.eapi().has(Feature::ParallelTests) {
                return emake(&[target]);
            } else {
                return emake(&["-j1", target]);
            }
        }
    }

    Ok(ExecStatus::Success)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::eapi;
    use crate::pkg::BuildablePackage;
    use crate::shell::test::FileTree;

    use super::*;

    #[test]
    fn test_src_install() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // default src_install is a no-op
        for eapi in eapi::range("0..4").unwrap() {
            for (s1, s2) in [("( a.txt )", "( a.html )"), ("\"a.txt\"", "\"a.html\"")] {
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="src_install installing docs"
                    SLOT=0
                    DOCS={s1}
                    HTML_DOCS={s2}
                "#};
                let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                BuildData::from_pkg(&pkg);
                let file_tree = FileTree::new();
                fs::write("a.txt", "data").unwrap();
                fs::write("a.html", "data").unwrap();
                pkg.build().unwrap();
                assert!(file_tree.is_empty());
            }
        }
    }
}
