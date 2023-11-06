use criterion::Criterion;

use pkgcraft::dep::{Dep, DepFields};

pub fn bench_pkg_deps(c: &mut Criterion) {
    c.bench_function("dep-parse-unversioned", |b| b.iter(|| Dep::new("cat/pkg")));

    c.bench_function("dep-parse-slotdep", |b| b.iter(|| Dep::new("cat/pkg:0")));

    c.bench_function("dep-parse-versioned", |b| b.iter(|| Dep::new(">=cat/pkg-4-r1")));

    c.bench_function("dep-parse-versioned-slotdep", |b| {
        b.iter(|| Dep::new(">=cat/pkg-4-r1:0="))
    });

    c.bench_function("dep-parse-usedeps", |b| {
        b.iter(|| Dep::new(">=cat/pkg-4-r1:0=[a,b=,!c=,d?,!e?,-f]"))
    });

    c.bench_function("dep-parse-long-usedeps", |b| {
        let flags: Vec<String> = (0..100).map(|s| s.to_string()).collect();
        let s = format!("cat/pkg[{}]", &flags.join(","));
        b.iter(|| Dep::new(&s));
    });

    c.bench_function("dep-cmp-eq", |b| {
        let d1 = Dep::new("=cat/pkg-1.2.3").unwrap();
        let d2 = Dep::new("=cat/pkg-1.2.3").unwrap();
        b.iter(|| d1 == d2);
    });

    c.bench_function("dep-cmp-lt", |b| {
        let d1 = Dep::new("=cat/pkg-1.2.3_alpha").unwrap();
        let d2 = Dep::new("=cat/pkg-1.2.3").unwrap();
        b.iter(|| d1 < d2);
    });

    c.bench_function("dep-cmp-gt", |b| {
        let d1 = Dep::new("=cat/pkg-1.2.3_p").unwrap();
        let d2 = Dep::new("=cat/pkg-1.2.3").unwrap();
        b.iter(|| d1 > d2);
    });

    c.bench_function("dep-valid", |b| {
        b.iter(|| Dep::valid(">=cat/pkg-1.2.3-r4:5/6=[a,b,c]", None))
    });

    c.bench_function("dep-cmp-sort", |b| {
        let mut deps: Vec<_> = (0..100)
            .rev()
            .map(|s| Dep::new(&format!("=cat/pkg-{}", s)).unwrap())
            .collect();
        b.iter(|| deps.sort());
    });

    c.bench_function("dep-without-owned", |b| {
        let dep = Dep::new("!!>=cat/pkg-1.2-r3:4/5=[a,b]::repo").unwrap();
        b.iter(|| dep.without(DepFields::UseDeps));
    });

    c.bench_function("dep-without-borrowed", |b| {
        let dep = Dep::new("!!>=cat/pkg-1.2-r3:4/5=[a,b]").unwrap();
        b.iter(|| dep.without(DepFields::Repo));
    });

    c.bench_function("dep-without-all", |b| {
        let dep = Dep::new("!!>=cat/pkg-1.2-r3:4/5=[a,b]::repo").unwrap();
        b.iter(|| dep.without(DepFields::all()));
    });

    c.bench_function("dep-without-empty", |b| {
        let dep = Dep::new("!!>=cat/pkg-1.2-r3:4/5=[a,b]").unwrap();
        b.iter(|| dep.without(DepFields::empty()));
    });
}
