use scallop::builtins::ExecStatus;
use scallop::variables::var_to_vec;

use crate::shell::builtins::{
    eapply::run as eapply, eapply_user::run as eapply_user, einstalldocs::run as einstalldocs,
};
use crate::shell::BuildData;

use super::emake_install;

pub(crate) fn src_prepare(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    if let Ok(patches) = var_to_vec("PATCHES") {
        if !patches.is_empty() {
            // Note that not allowing options in PATCHES is technically from EAPI 8, but it's
            // backported here for EAPI 6 onwards.
            let mut args = vec!["--"];
            // TODO: need to perform word expansion on each string as well
            args.extend(patches.iter().map(|s| s.as_str()));
            eapply(&args)?;
        }
    }
    eapply_user(&[])
}

pub(crate) fn src_install(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    emake_install(build)?;
    einstalldocs(&[])
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::eapi;
    use crate::macros::assert_err_re;
    use crate::pkg::BuildablePackage;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;

    #[test]
    fn src_prepare() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        let file_content: &str = indoc::indoc! {"
            0
        "};
        let patch1: &str = indoc::indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -0
            +1
        "};
        let patch2: &str = indoc::indoc! {"
            --- a/file.txt
            +++ a/file.txt
            @@ -1 +1 @@
            -1
            +2
        "};

        for eapi in eapi::range("6..").unwrap() {
            // no options in PATCHES
            for s in ["( -p1 1.patch )", "\"-p1 1.patch\""] {
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="PATCHES empty"
                    SLOT=0
                    PATCHES={s}
                "#};
                let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                BuildData::from_pkg(&pkg);
                let r = pkg.build();
                assert_err_re!(r, "nonexistent file: -p1$");
            }

            // PATCHES empty
            for s in ["()", "\"\"", ""] {
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="PATCHES empty"
                    SLOT=0
                    PATCHES={s}
                "#};
                let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                BuildData::from_pkg(&pkg);
                let _file_tree = FileTree::new();
                fs::write("file.txt", file_content).unwrap();
                fs::write("1.patch", patch1).unwrap();
                fs::write("2.patch", patch2).unwrap();
                pkg.build().unwrap();
                assert_eq!(fs::read_to_string("file.txt").unwrap(), "0\n");
            }

            // PATCHES applied
            for s in ["( 1.patch 2.patch )", "\"1.patch 2.patch\""] {
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="PATCHES array"
                    SLOT=0
                    PATCHES={s}
                "#};
                let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                BuildData::from_pkg(&pkg);
                let _file_tree = FileTree::new();
                fs::write("file.txt", file_content).unwrap();
                fs::write("1.patch", patch1).unwrap();
                fs::write("2.patch", patch2).unwrap();
                pkg.build().unwrap();
                assert_eq!(fs::read_to_string("file.txt").unwrap(), "2\n");
            }
        }
    }
}
