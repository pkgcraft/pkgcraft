use scallop::Error;

use crate::dep::Cpv;
use crate::pkg::Package;
use crate::repo::PkgRepository;
use crate::shell::get_build_mut;

/// Underlying query support for has_version and best_version.
pub(crate) fn query_cmd(args: &[&str]) -> scallop::Result<impl Iterator<Item = Cpv>> {
    let build = get_build_mut();
    // TODO: add options parsing support
    let dep = match args[..] {
        [s] => build.eapi().dep(s)?,
        _ => return Err(Error::Base(format!("requires 1 arg, got {}", args.len()))),
    };

    // TODO: use the build config's install repo
    Ok(build.repo()?.iter_restrict(&dep).map(|p| p.cpv().clone()))
}
