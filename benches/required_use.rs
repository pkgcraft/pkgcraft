use criterion::Criterion;

use pkgcraft::depspec::required_use;
use pkgcraft::eapi::EAPI_LATEST;

#[allow(unused_must_use)]
pub fn bench_parse_required_use(c: &mut Criterion) {
    c.bench_function("required-use-conditional", |b| {
        b.iter(|| {
            required_use::parse("u1? ( u2 )", &EAPI_LATEST);
        })
    });
}
