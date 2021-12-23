use criterion::Criterion;

use pkgcraft::eapi::EAPI_LATEST;

#[allow(unused_must_use)]
pub fn bench_parse_unversioned(c: &mut Criterion) {
    c.bench_function("atom-unversioned", |b| {
        b.iter(|| EAPI_LATEST.atom("cat/pkg"))
    });

    c.bench_function("atom-slotdep", |b| b.iter(|| EAPI_LATEST.atom("cat/pkg:0")));

    c.bench_function("atom-versioned", |b| {
        b.iter(|| EAPI_LATEST.atom(">=cat/pkg-4-r1"))
    });

    c.bench_function("atom-versioned-slotdep", |b| {
        b.iter(|| EAPI_LATEST.atom(">=cat/pkg-4-r1:0="))
    });

    c.bench_function("atom-usedeps", |b| {
        b.iter(|| EAPI_LATEST.atom(">=cat/pkg-4-r1:0=[a,b=,!c=,d?,!e?,-f]"))
    });

    c.bench_function("atom-long-usedeps", |b| {
        let flags: Vec<String> = (0..100).map(|s| s.to_string()).collect();
        let s = format!("cat/pkg[{}]", &flags.join(","));
        b.iter(|| EAPI_LATEST.atom(&s));
    });

    c.bench_function("atom-sorting", |b| {
        let mut atoms: Vec<_> = (0..100)
            .map(|s| EAPI_LATEST.atom(&format!("=cat/pkg-{}", s)).unwrap())
            .collect();
        b.iter(|| atoms.sort());
    });
}
