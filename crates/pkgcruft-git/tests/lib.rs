mod client;
mod git;
mod pre_commit;
mod pre_push;
mod pre_receive;
mod prepare_commit_msg;
mod server;
mod service;

pkgcraft::test::define_cmd!("pkgcruft-gitd", "pkgcruft-git");
