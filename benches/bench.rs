use criterion::*;

mod atom;
mod required_use;
mod version;

criterion_group!(atom, atom::bench_pkg_atoms);
criterion_group!(required_use, required_use::bench_parse_required_use);
criterion_group!(version, version::bench_pkg_versions);

criterion_main!(atom, required_use, version);
