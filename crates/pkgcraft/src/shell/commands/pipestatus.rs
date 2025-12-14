use std::io::Write;

use itertools::Itertools;
use scallop::ExecStatus;
use scallop::array::PipeStatus;

use crate::io::stdout;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(name = "pipestatus", long_about = "Support PIPESTATUS assertions.")]
struct Command {
    #[arg(short = 'v')]
    verbose: bool,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;

    let pipestatus = PipeStatus::get();
    if cmd.verbose {
        writeln!(stdout(), "{}", pipestatus.iter().join(" "))?;
    }

    Ok(pipestatus.status().into())
}

make_builtin!("pipestatus", pipestatus_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::pkg::Build;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;

    use scallop::source;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::pipestatus};
    use super::*;

    cmd_scope_tests!("pipestatus");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(pipestatus, &[1]);
    }

    #[test]
    fn success() {
        source::string("true | true").unwrap();
        assert_eq!(pipestatus(&[]).unwrap(), ExecStatus::Success);
        assert_eq!(stdout().get(), "");
        assert_eq!(pipestatus(&["-v"]).unwrap(), ExecStatus::Success);
        assert_eq!(stdout().get(), "0 0\n");
    }

    #[test]
    fn failure() {
        source::string("true | false | true").unwrap();
        assert_eq!(pipestatus(&[]).unwrap(), ExecStatus::Failure(1));
        assert_eq!(stdout().get(), "");
        assert_eq!(pipestatus(&["-v"]).unwrap(), ExecStatus::Failure(1));
        assert_eq!(stdout().get(), "0 1 0\n");

        // custom status
        source::string(indoc::indoc! {"
            func1() {
                return 1
            }

            func2() {
                return 123
            }

            func1 | func2 | true
        "})
        .unwrap();
        assert_eq!(pipestatus(&[]).unwrap(), ExecStatus::Failure(123));
        assert_eq!(stdout().get(), "");
        assert_eq!(pipestatus(&["-v"]).unwrap(), ExecStatus::Failure(123));
        assert_eq!(stdout().get(), "1 123 0\n");
    }

    #[test]
    fn ebuild() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        let data = indoc::indoc! {r#"
            EAPI=9
            DESCRIPTION="testing pipestatus"
            SLOT=0

            func1() {
                return 1
            }

            func2() {
                return 123
            }

            src_configure() {
                # success
                true | true | true
                pipestatus -v; status=$?
                [[ ${status} == 0 ]] || die "invalid pipestatus success status: ${status}"

                # failure
                func1 | func2 | true
                pipestatus -v; status=$?
                [[ ${status} == 123 ]] || die "invalid pipestatus failure status: ${status}"
            }
        "#};

        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        let r = pkg.build();
        assert!(r.is_ok(), "{}", r.unwrap_err());
        assert_eq!(stdout().get(), "0 0 0\n1 123 0\n");
    }
}
