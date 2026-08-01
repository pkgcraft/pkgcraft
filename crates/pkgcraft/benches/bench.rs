use criterion::*;

pub mod config;
pub mod dep;
pub mod depset;
pub mod repo;
pub mod version;

criterion_group!(config, config::bench_config);
criterion_group!(dep, dep::bench_pkg_deps);
criterion_group!(depset, depset::bench_depsets);
criterion_group!(repo, repo::bench_repo_ebuild);
criterion_group!(version, version::bench_pkg_versions);

criterion_main!(config, dep, repo, depset, version);
