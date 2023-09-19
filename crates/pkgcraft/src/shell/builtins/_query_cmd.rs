use scallop::Error;

use crate::dep::Cpv;
use crate::eapi::Feature::{QueryDeps, QueryHostRoot};
use crate::pkg::Package;
use crate::repo::PkgRepository;
use crate::shell::get_build_mut;

/// Underlying query support for has_version and best_version.
pub(crate) fn query_cmd(args: &[&str]) -> scallop::Result<impl Iterator<Item = Cpv>> {
    let build = get_build_mut();
    let eapi = build.eapi();

    // TODO: add proper root mapping support
    let (_root, dep) = match args[..] {
        [s] => ("/", s),
        [opt, s] if opt == "--host-root" && eapi.has(QueryHostRoot) => ("/", s),
        [opt, s] if opt == "-b" && eapi.has(QueryDeps) => ("/", s),
        [opt, s] if opt == "-d" && eapi.has(QueryDeps) => ("/", s),
        [opt, s] if opt == "-r" && eapi.has(QueryDeps) => ("/", s),
        [opt, _] => return Err(Error::Base(format!("{opt} unsupported in EAPI {eapi}"))),
        _ => return Err(Error::Base(format!("requires 1 arg, got {}", args.len()))),
    };

    let dep = build.eapi().dep(dep)?;

    // TODO: pull the install repo related to the root setting
    Ok(build.repo()?.iter_restrict(&dep).map(|p| p.cpv().clone()))
}
