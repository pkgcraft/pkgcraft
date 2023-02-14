use std::str::FromStr;

use criterion::Criterion;

use pkgcraft::dep::Dep;

pub fn bench_pkg_deps(c: &mut Criterion) {
    c.bench_function("dep-parse-unversioned", |b| b.iter(|| Dep::from_str("cat/pkg")));

    c.bench_function("dep-parse-slotdep", |b| b.iter(|| Dep::from_str("cat/pkg:0")));

    c.bench_function("dep-parse-versioned", |b| b.iter(|| Dep::from_str(">=cat/pkg-4-r1")));

    c.bench_function("dep-parse-versioned-slotdep", |b| {
        b.iter(|| Dep::from_str(">=cat/pkg-4-r1:0="))
    });

    c.bench_function("dep-parse-usedeps", |b| {
        b.iter(|| Dep::from_str(">=cat/pkg-4-r1:0=[a,b=,!c=,d?,!e?,-f]"))
    });

    c.bench_function("dep-parse-long-usedeps", |b| {
        let flags: Vec<String> = (0..100).map(|s| s.to_string()).collect();
        let s = format!("cat/pkg[{}]", &flags.join(","));
        b.iter(|| Dep::from_str(&s));
    });

    c.bench_function("dep-cmp-eq", |b| {
        let d1 = Dep::from_str("=cat/pkg-1.2.3").unwrap();
        let d2 = Dep::from_str("=cat/pkg-1.2.3").unwrap();
        b.iter(|| d1 == d2);
    });

    c.bench_function("dep-cmp-lt", |b| {
        let d1 = Dep::from_str("=cat/pkg-1.2.3_alpha").unwrap();
        let d2 = Dep::from_str("=cat/pkg-1.2.3").unwrap();
        b.iter(|| d1 < d2);
    });

    c.bench_function("dep-cmp-gt", |b| {
        let d1 = Dep::from_str("=cat/pkg-1.2.3_p").unwrap();
        let d2 = Dep::from_str("=cat/pkg-1.2.3").unwrap();
        b.iter(|| d1 > d2);
    });

    c.bench_function("dep-cmp-sort", |b| {
        let mut deps: Vec<_> = (0..100)
            .rev()
            .map(|s| Dep::from_str(&format!("=cat/pkg-{}", s)).unwrap())
            .collect();
        b.iter(|| deps.sort());
    });
}
