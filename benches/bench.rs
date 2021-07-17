use criterion::*;

mod atom;
mod required_use;

criterion_group!(atom, atom::bench_parse_unversioned);
criterion_group!(required_use, required_use::bench_parse_required_use);

criterion_main!(atom, required_use);
