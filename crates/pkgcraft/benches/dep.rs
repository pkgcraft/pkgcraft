use criterion::Criterion;

use pkgcraft::dep::{Dep, DepField};

pub fn bench_pkg_deps(c: &mut Criterion) {
    c.bench_function("dep-parse", |b| {
        b.iter(|| Dep::parse(">=cat/pkg-1.2.3-r4:5/6=[a,-b,c?]", None))
    });

    c.bench_function("dep-new", |b| b.iter(|| Dep::new(">=cat/pkg-1.2.3-r4:5/6=[a,-b,c?]")));

    c.bench_function("dep-unversioned", |b| b.iter(|| Dep::new("cat/pkg")));

    c.bench_function("dep-slotdep", |b| b.iter(|| Dep::new("cat/pkg:0")));

    c.bench_function("dep-versioned", |b| b.iter(|| Dep::new(">=cat/pkg-4-r1")));

    c.bench_function("dep-versioned-slotdep", |b| b.iter(|| Dep::new(">=cat/pkg-4-r1:0=")));

    c.bench_function("dep-usedeps", |b| {
        b.iter(|| Dep::new(">=cat/pkg-4-r1:0=[a,b=,!c=,d?,!e?,-f]"))
    });

    c.bench_function("dep-long-usedeps", |b| {
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

    c.bench_function("dep-cmp-sort", |b| {
        let mut deps: Vec<_> = (0..100)
            .rev()
            .map(|s| Dep::new(&format!("=cat/pkg-{}", s)).unwrap())
            .collect();
        b.iter(|| deps.sort());
    });

    c.bench_function("dep-without-owned", |b| {
        let dep = Dep::new("!!>=cat/pkg-1.2-r3:4/5=::repo[a,b]").unwrap();
        b.iter(|| dep.without([DepField::UseDeps]));
    });

    c.bench_function("dep-without-borrowed", |b| {
        let dep = Dep::new("!!>=cat/pkg-1.2-r3:4/5=[a,b]").unwrap();
        b.iter(|| dep.without([DepField::Repo]));
    });

    c.bench_function("dep-without-all", |b| {
        let dep = Dep::new("!!>=cat/pkg-1.2-r3:4/5=::repo[a,b]").unwrap();
        b.iter(|| dep.without(DepField::optional()));
    });
}
