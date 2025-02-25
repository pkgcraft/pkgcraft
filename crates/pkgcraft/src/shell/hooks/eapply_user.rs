use scallop::{Error, ExecStatus};

use crate::shell::BuildData;

pub(crate) fn post(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    if !build.user_patches_applied {
        Err(Error::Base("eapply_user was not called during src_prepare()".to_string()))
    } else {
        Ok(ExecStatus::Success)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::pkg::Build;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;

    #[test]
    fn called() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        let data = indoc::formatdoc! {r#"
            EAPI=8
            DESCRIPTION="eapply_user called by default"
            SLOT=0
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        assert!(pkg.build().is_ok());

        let data = indoc::formatdoc! {r#"
            EAPI=8
            DESCRIPTION="eapply_user called via default"
            SLOT=0
            src_prepare() {{
                default
            }}
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        assert!(pkg.build().is_ok());

        let data = indoc::formatdoc! {r#"
            EAPI=8
            DESCRIPTION="eapply_user called explicitly"
            SLOT=0
            src_prepare() {{
                eapply_user
            }}
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        assert!(pkg.build().is_ok());
    }

    #[test]
    fn uncalled() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        let data = indoc::formatdoc! {r#"
            EAPI=8
            DESCRIPTION="eapply_user uncalled"
            SLOT=0
            src_prepare() {{
                :
            }}
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        let r = pkg.build();
        assert_err_re!(r, "eapply_user was not called during src_prepare()");
    }
}
