use criterion::*;

mod atom;
mod repo;
mod required_use;
mod version;

criterion_group!(atom, atom::bench_pkg_atoms);
criterion_group!(repo, repo::bench_repo_ebuild);
criterion_group!(required_use, required_use::bench_parse_required_use);
criterion_group!(version, version::bench_pkg_versions);

criterion_main!(atom, repo, required_use, version);
