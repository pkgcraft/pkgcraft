use std::str::FromStr;

use criterion::Criterion;

use pkgcraft::dep::PkgDep;

pub fn bench_pkg_deps(c: &mut Criterion) {
    c.bench_function("pkgdep-parse-unversioned", |b| b.iter(|| PkgDep::from_str("cat/pkg")));

    c.bench_function("pkgdep-parse-slotdep", |b| b.iter(|| PkgDep::from_str("cat/pkg:0")));

    c.bench_function("pkgdep-parse-versioned", |b| b.iter(|| PkgDep::from_str(">=cat/pkg-4-r1")));

    c.bench_function("pkgdep-parse-versioned-slotdep", |b| {
        b.iter(|| PkgDep::from_str(">=cat/pkg-4-r1:0="))
    });

    c.bench_function("pkgdep-parse-usedeps", |b| {
        b.iter(|| PkgDep::from_str(">=cat/pkg-4-r1:0=[a,b=,!c=,d?,!e?,-f]"))
    });

    c.bench_function("pkgdep-parse-long-usedeps", |b| {
        let flags: Vec<String> = (0..100).map(|s| s.to_string()).collect();
        let s = format!("cat/pkg[{}]", &flags.join(","));
        b.iter(|| PkgDep::from_str(&s));
    });

    c.bench_function("pkgdep-cmp-eq", |b| {
        let a1 = PkgDep::from_str("=cat/pkg-1.2.3").unwrap();
        let a2 = PkgDep::from_str("=cat/pkg-1.2.3").unwrap();
        b.iter(|| a1 == a2);
    });

    c.bench_function("pkgdep-cmp-lt", |b| {
        let a1 = PkgDep::from_str("=cat/pkg-1.2.3_alpha").unwrap();
        let a2 = PkgDep::from_str("=cat/pkg-1.2.3").unwrap();
        b.iter(|| a1 < a2);
    });

    c.bench_function("pkgdep-cmp-gt", |b| {
        let a1 = PkgDep::from_str("=cat/pkg-1.2.3_p").unwrap();
        let a2 = PkgDep::from_str("=cat/pkg-1.2.3").unwrap();
        b.iter(|| a1 > a2);
    });

    c.bench_function("pkgdep-cmp-sort", |b| {
        let mut deps: Vec<_> = (0..100)
            .rev()
            .map(|s| PkgDep::from_str(&format!("=cat/pkg-{}", s)).unwrap())
            .collect();
        b.iter(|| deps.sort());
    });
}
