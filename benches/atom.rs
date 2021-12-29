use std::str::FromStr;

use criterion::Criterion;

use pkgcraft::atom::Atom;

#[allow(unused_must_use)]
pub fn bench_parse_unversioned(c: &mut Criterion) {
    c.bench_function("atom-parse-unversioned", |b| {
        b.iter(|| Atom::from_str("cat/pkg"))
    });

    c.bench_function("atom-parse-slotdep", |b| {
        b.iter(|| Atom::from_str("cat/pkg:0"))
    });

    c.bench_function("atom-parse-versioned", |b| {
        b.iter(|| Atom::from_str(">=cat/pkg-4-r1"))
    });

    c.bench_function("atom-parse-versioned-slotdep", |b| {
        b.iter(|| Atom::from_str(">=cat/pkg-4-r1:0="))
    });

    c.bench_function("atom-parse-usedeps", |b| {
        b.iter(|| Atom::from_str(">=cat/pkg-4-r1:0=[a,b=,!c=,d?,!e?,-f]"))
    });

    c.bench_function("atom-parse-long-usedeps", |b| {
        let flags: Vec<String> = (0..100).map(|s| s.to_string()).collect();
        let s = format!("cat/pkg[{}]", &flags.join(","));
        b.iter(|| Atom::from_str(&s));
    });

    c.bench_function("atom-cmp-eq", |b| {
        let a1 = Atom::from_str("=cat/pkg-1.2.3").unwrap();
        let a2 = Atom::from_str("=cat/pkg-1.2.3").unwrap();
        b.iter(|| a1 == a2);
    });

    c.bench_function("atom-cmp-lt", |b| {
        let a1 = Atom::from_str("=cat/pkg-1.2.3_alpha").unwrap();
        let a2 = Atom::from_str("=cat/pkg-1.2.3").unwrap();
        b.iter(|| a1 < a2);
    });

    c.bench_function("atom-cmp-gt", |b| {
        let a1 = Atom::from_str("=cat/pkg-1.2.3_p").unwrap();
        let a2 = Atom::from_str("=cat/pkg-1.2.3").unwrap();
        b.iter(|| a1 > a2);
    });

    c.bench_function("atom-cmp-sort", |b| {
        let mut atoms: Vec<_> = (0..100)
            .rev()
            .map(|s| Atom::from_str(&format!("=cat/pkg-{}", s)).unwrap())
            .collect();
        b.iter(|| atoms.sort());
    });
}
