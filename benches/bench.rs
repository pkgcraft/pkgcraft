use criterion::*;

mod atom;
mod depspec;
mod repo;
mod version;

criterion_group!(atom, atom::bench_pkg_atoms);
criterion_group!(repo, repo::bench_repo_ebuild);
criterion_group!(depspec, depspec::bench_parse_required_use);
criterion_group!(version, version::bench_pkg_versions);

criterion_main!(atom, repo, depspec, version);
