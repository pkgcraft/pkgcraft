use std::io::Write;

use scallop::ExecStatus;

use crate::io::stdout;
use crate::shell::get_build_mut;

use super::{TryParseArgs, make_builtin, parse};

#[derive(clap::Parser, Debug)]
#[command(
    name = "ver_rs",
    disable_help_flag = true,
    long_about = "Perform string substitution on package version strings."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = true, allow_hyphen_values = true, num_args = 2..)]
    args: Vec<String>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let mut cmd = Command::try_parse_args(args)?;
    let version = if cmd.args.len() % 2 == 0 {
        get_build_mut().cpv().pv()
    } else {
        cmd.args.pop().unwrap()
    };

    // split version string into separators and components, note that invalid versions
    // like ".1.2.3" are allowed
    let mut version_parts = parse::version_split(&version)?;
    let len = version_parts.len();

    // iterate over (range, separator) pairs, altering the denoted separators as requested
    let mut args_iter = cmd.args.chunks_exact(2);
    while let Some([range, sep]) = args_iter.next() {
        let (start, end) = parse::range(range, len / 2)?;
        (start..=end)
            .map(|i| i * 2)
            .take_while(|&i| i < len)
            .for_each(|i| {
                if (i > 0 && i < len - 1) || !version_parts[i].is_empty() {
                    version_parts[i] = sep;
                }
            });
    }

    let mut stdout = stdout();
    write!(stdout, "{}", version_parts.join(""))?;
    stdout.flush()?;

    Ok(ExecStatus::Success)
}

make_builtin!("ver_rs", ver_rs_builtin);

#[cfg(test)]
mod tests {
    use scallop::source;

    use crate::config::Config;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, ver_rs};
    use super::*;

    cmd_scope_tests!("ver_rs 2 - 1.2.3");

    #[test]
    fn invalid_args() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);
        assert_invalid_cmd(ver_rs, &[0, 1]);
    }

    #[test]
    fn invalid_range() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        for rng in ["-", "-2"] {
            let r = ver_rs(&[rng, "2", "1.2.3"]);
            assert!(r.unwrap_err().to_string().contains("invalid range"));
        }

        let r = ver_rs(&["3-2", "1", "1.2.3"]);
        assert_err_re!(r, " is greater than end ");
    }

    #[test]
    fn output() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        // invalid PV
        for (args, expected) in [
            (vec!["1", "-", ".1.2.3"], ".1-2.3"),
            (vec!["0", "-", ".1.2.3"], "-1.2.3"),
            (vec!["2", ".", "1.2-3"], "1.2.3"),
            (vec!["3-5", "_", "4-6", "-", "a1b2c3d4e5"], "a1b_2-c-3-d4e5"),
        ] {
            temp.create_ebuild("cat/pkg-1.2.3", &[]).unwrap();
            let raw_pkg = repo.get_pkg_raw("cat/pkg-1.2.3").unwrap();
            BuildData::from_raw_pkg(&raw_pkg);

            let r = ver_rs(&args).unwrap();
            assert_eq!(stdout().get(), expected);
            assert_eq!(r, ExecStatus::Success);
        }

        // valid PV
        for (mut args, expected) in [
            (vec!["1", "-", "1.2.3"], "1-2.3"),
            (vec!["2", "-", "1.2.3"], "1.2-3"),
            (vec!["1-2", "-", "1.2.3.4"], "1-2-3.4"),
            (vec!["2-", "-", "1.2.3.4"], "1.2-3-4"),
            (vec!["3", ".", "1.2.3a"], "1.2.3.a"),
            (vec!["2-3", "-", "1.2_alpha4"], "1.2-alpha-4"),
            (vec!["3", "-", "2", "", "1.2.3b_alpha4"], "1.23-b_alpha4"),
            (vec!["0", "-", "1.2.3"], "1.2.3"),
            (vec!["3", ".", "1.2.3"], "1.2.3"),
            (vec!["3-", ".", "1.2.3"], "1.2.3"),
            (vec!["3-5", ".", "1.2.3"], "1.2.3"),
        ] {
            let ver = args.last().unwrap();
            temp.create_ebuild(format!("cat/pkg-{ver}"), &[]).unwrap();
            let raw_pkg = repo.get_pkg_raw(format!("cat/pkg-{ver}")).unwrap();
            BuildData::from_raw_pkg(&raw_pkg);

            let r = ver_rs(&args).unwrap();
            assert_eq!(stdout().get(), expected);
            assert_eq!(r, ExecStatus::Success);

            // test pulling version from $PV
            args.pop();
            let r = ver_rs(&args).unwrap();
            assert_eq!(stdout().get(), expected);
            assert_eq!(r, ExecStatus::Success);
        }
    }

    #[ignore]
    #[test]
    fn subshell() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat/pkg-1.2.3", &[]).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1.2.3").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        source::string("VER=$(ver_rs 2 - 1.2.3)").unwrap();
        assert_eq!(scallop::variables::optional("VER").unwrap(), "1.2-3");

        // test pulling version from $PV
        source::string("VER=$(ver_rs 1 -)").unwrap();
        assert_eq!(scallop::variables::optional("VER").unwrap(), "1-2.3");
    }
}
