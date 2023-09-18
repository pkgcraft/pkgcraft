use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkg::Package;
use crate::repo::PkgRepository;
use crate::shell::{get_build_mut, write_stdout};

use super::{make_builtin, Scopes::Phases};

const LONG_DOC: &str = "Output the highest matching version of a package dependency is installed.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let build = get_build_mut();
    let dep = match args[..] {
        [s] => build.eapi().dep(s)?,
        _ => return Err(Error::Base(format!("requires 1 arg, got {}", args.len()))),
    };

    // TODO: use the build config's install repo
    let mut pkgs: Vec<_> = build.repo()?.iter_restrict(&dep).collect();
    pkgs.sort();

    if let Some(pkg) = pkgs.last() {
        write_stdout!("{}", pkg.cpv())?;
        Ok(ExecStatus::Success)
    } else {
        write_stdout!("")?;
        Ok(ExecStatus::Failure(1))
    }
}

const USAGE: &str = "best_version cat/pkg";
make_builtin!("best_version", best_version_builtin, run, LONG_DOC, USAGE, [("..", [Phases])]);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as best_version;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(best_version, &[0]);
    }

    // TODO: add usage tests
}
