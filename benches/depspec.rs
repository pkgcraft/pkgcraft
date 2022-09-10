use criterion::Criterion;

use pkgcraft::depspec::parse;
use pkgcraft::eapi::EAPI_LATEST;

#[allow(unused_must_use)]
pub fn bench_parse_required_use(c: &mut Criterion) {
    c.bench_function("required-use-conditional", |b| {
        b.iter(|| {
            parse::required_use("u1? ( u2 )", &EAPI_LATEST);
        })
    });
}
