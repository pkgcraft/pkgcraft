use criterion::{criterion_group, criterion_main, Criterion};

use arcanist::depspec::required_use;
use arcanist::eapi::EAPI_LATEST;

#[allow(unused_must_use)]
fn bench_parse_required_use(c: &mut Criterion) {
    c.bench_function("required-use-conditional", |b| b.iter(|| {
        required_use::parse("u1? ( u2 )", EAPI_LATEST);
    }));
}

criterion_group!(required_use_benches, bench_parse_required_use);
criterion_main!(required_use_benches);
