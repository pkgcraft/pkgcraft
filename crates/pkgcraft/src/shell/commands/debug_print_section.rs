use scallop::ExecStatus;
use tracing::debug;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "debug-print-section",
    disable_help_flag = true,
    long_about = "Calls debug-print with `now in section $*`."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    debug!("now in section {}", cmd.args.join(" "));
    Ok(ExecStatus::Success)
}

make_builtin!("debug-print-section", debug_print_section_builtin);

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use crate::config::Config;
    use crate::pkg::Source;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::test::assert_logs_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::debug_print_section};

    cmd_scope_tests!("debug-print-section arg1 arg2");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(debug_print_section, &[0]);
    }

    #[traced_test]
    #[test]
    fn eclass() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        let eclass = indoc::indoc! {r#"
            # stub eclass
            e1_func() {
                debug-print-section section1 "$@"
            }
            e1_func msg 1 2 3
        "#};
        temp.create_eclass("e1", eclass).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing debug-print-section"
            SLOT=0
        "#};

        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        raw_pkg.source().unwrap();
        assert_logs_re!("now in section section1 msg 1 2 3$");
    }

    #[traced_test]
    #[test]
    fn global() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        let eclass = indoc::indoc! {r#"
            # stub eclass
            e1_func() {
                debug-print-section section1 "$@"
            }
        "#};
        temp.create_eclass("e1", eclass).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing debug-print-section"
            SLOT=0
            e1_func msg 1 2 3
        "#};

        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        raw_pkg.source().unwrap();
        assert_logs_re!("now in section section1 msg 1 2 3$");
    }
}
