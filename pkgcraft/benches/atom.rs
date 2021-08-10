use std::str::FromStr;

use criterion::Criterion;

use pkgcraft::atom;
use pkgcraft::eapi::EAPI_LATEST;

#[allow(unused_must_use)]
pub fn bench_parse_unversioned(c: &mut Criterion) {
    c.bench_function("atom-unversioned", |b| {
        b.iter(|| {
            atom::parse::dep("cat/pkg", EAPI_LATEST);
        })
    });

    c.bench_function("atom-slotdep", |b| {
        b.iter(|| {
            atom::parse::dep("cat/pkg:0", EAPI_LATEST);
        })
    });

    c.bench_function("atom-versioned", |b| {
        b.iter(|| {
            atom::parse::dep(">=cat/pkg-4-r1", EAPI_LATEST);
        })
    });

    c.bench_function("atom-versioned-slotdep", |b| {
        b.iter(|| {
            atom::parse::dep(">=cat/pkg-4-r1:0=", EAPI_LATEST);
        })
    });

    c.bench_function("atom-usedeps", |b| {
        b.iter(|| {
            atom::parse::dep(">=cat/pkg-4-r1:0=[a,b=,!c=,d?,!e?,-f]", EAPI_LATEST);
        })
    });

    c.bench_function("atom-long-usedeps", |b| {
        let flags: Vec<String> = (0..100).map(|s| s.to_string()).collect();
        let s = format!("cat/pkg[{}]", &flags.join(","));
        b.iter(|| atom::parse::dep(&s, EAPI_LATEST));
    });

    c.bench_function("atom-sorting", |b| {
        let mut atoms: Vec<atom::Atom> = (0..100)
            .map(|s| atom::Atom::from_str(&format!("=cat/pkg-{}", s)).unwrap())
            .collect();
        b.iter(|| atoms.sort());
    });
}
