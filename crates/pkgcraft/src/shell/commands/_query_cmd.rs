use scallop::Error;

use crate::dep::Cpv;
use crate::eapi::Feature::{QueryDeps, QueryHostRoot};
use crate::repo::PkgRepository;
use crate::shell::get_build_mut;

/// Underlying query support for has_version and best_version.
pub(crate) fn query_cmd(args: &[&str]) -> scallop::Result<Vec<Cpv>> {
    let build = get_build_mut();
    let eapi = build.eapi();

    // TODO: add proper root mapping support
    let (_root, dep) = match args[..] {
        [s] => ("/", s),
        ["--host-root", s] if eapi.has(QueryHostRoot) => ("/", s),
        ["-b", s] if eapi.has(QueryDeps) => ("/", s),
        ["-d", s] if eapi.has(QueryDeps) => ("/", s),
        ["-r", s] if eapi.has(QueryDeps) => ("/", s),
        _ => return Err(Error::Base("invalid args, see PMS for details".to_string())),
    };

    let dep = eapi.dep(dep)?;

    // TODO: pull the install repo related to the root setting
    Ok(build.repo()?.iter_cpv_restrict(&dep).collect())
}
