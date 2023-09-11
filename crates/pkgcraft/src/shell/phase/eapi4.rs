use scallop::builtins::ExecStatus;

use crate::shell::builtins::einstalldocs::install_docs_from;
use crate::shell::BuildData;

use super::emake_install;

pub(crate) fn src_install(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    emake_install(build)?;
    install_docs_from("DOCS")
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

        // default src_install only handles DOCS and not HTML_DOCS
        for eapi in eapi::range("4..6").unwrap() {
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
                file_tree.assert(
                    r#"
                    [[files]]
                    path = "/usr/share/doc/pkg-1/a.txt"
                "#,
                );
            }
        }
    }
}
