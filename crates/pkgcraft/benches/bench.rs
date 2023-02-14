use criterion::*;

mod dep;
mod depset;
mod repo;
mod version;

criterion_group!(dep, dep::bench_pkg_deps);
criterion_group!(repo, repo::bench_repo_ebuild);
criterion_group!(depset, depset::bench_depsets);
criterion_group!(version, version::bench_pkg_versions);

criterion_main!(dep, repo, depset, version);
