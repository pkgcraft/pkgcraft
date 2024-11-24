use scallop::ExecStatus;
use tracing::debug;

use super::make_builtin;

const LONG_DOC: &str = "\
If in a special debug mode, the arguments should be outputted or recorded using some kind of debug
logging.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    debug!("{}", args.join(" "));
    Ok(ExecStatus::Success)
}

const USAGE: &str = "debug-print msg";
make_builtin!("debug-print", debug_print_builtin);

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use crate::config::Config;
    use crate::macros::assert_logs_re;
    use crate::pkg::Source;
    use crate::repo::ebuild::EbuildRepoBuilder;

    use super::super::cmd_scope_tests;
    use super::*;

    cmd_scope_tests!(USAGE);

    #[traced_test]
    #[test]
    fn eclass() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        let eclass = indoc::indoc! {r#"
            # stub eclass
            debug-print "eclass: ${ECLASS}"
        "#};
        temp.create_eclass("e1", eclass).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing debug-print"
            SLOT=0
        "#};

        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        raw_pkg.source().unwrap();
        assert_logs_re!("eclass: e1$");
    }

    #[traced_test]
    #[test]
    fn global() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        let eclass = indoc::indoc! {r#"
            # stub eclass
            e1_func() {
                debug-print ${FUNCNAME}: "$@"
            }
        "#};
        temp.create_eclass("e1", eclass).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing debug-print"
            SLOT=0
            e1_func msg 1 2 3
        "#};

        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        raw_pkg.source().unwrap();
        assert_logs_re!("e1_func: msg 1 2 3$");
    }
}
