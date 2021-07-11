use criterion::{criterion_group, criterion_main, Criterion};

use arcanist::atom;
use arcanist::eapi;

#[allow(unused_must_use)]
fn bench_parse_unversioned(c: &mut Criterion) {
    c.bench_function("atom-unversioned", |b| b.iter(|| {
        atom::parse("cat/pkg", &eapi::EAPI1);
    }));

    c.bench_function("atom-slotdep", |b| b.iter(|| {
        atom::parse("cat/pkg:0", &eapi::EAPI5);
    }));

    c.bench_function("atom-versioned", |b| b.iter(|| {
        atom::parse(">=cat/pkg-4-r1", &eapi::EAPI1);
    }));

    c.bench_function("atom-versioned-slotdep", |b| b.iter(|| {
        atom::parse(">=cat/pkg-4-r1:0=", &eapi::EAPI5);
    }));

    c.bench_function("atom-usedeps", |b| b.iter(|| {
        atom::parse(">=cat/pkg-4-r1:0=[a,b=,!c=,d?,!e?,-f]", &eapi::EAPI5);
    }));

    c.bench_function("atom-long-usedeps", |b| {
        let flags: Vec<String> = (0..100).map(|s| s.to_string()).collect();
        let s = format!("cat/pkg[{}]", &flags.join(","));
        b.iter(|| atom::parse(&s, &eapi::EAPI5));
    });
}

criterion_group!(atom_benches, bench_parse_unversioned);
criterion_main!(atom_benches);
