mod client;
mod git;
mod pre_commit;
mod pre_push;
mod server;
mod service;

pkgcraft::test::define_cmd!("pkgcruft-gitd", "pkgcruft-git");
