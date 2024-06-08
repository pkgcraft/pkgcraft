use criterion::Criterion;

use pkgcraft::dep::{Dep, DependencySet};
use pkgcraft::eapi::EAPI_LATEST_OFFICIAL;
use pkgcraft::restrict::{Restrict, Restriction};

pub fn bench_depsets(c: &mut Criterion) {
    c.bench_function("depset-parse-required-use", |b| {
        b.iter(|| {
            DependencySet::required_use("u1? ( u2 )").unwrap();
        })
    });

    let deps = "c/p1 u1? ( c/p2 !u2? ( c/p3 ) ) || ( c/p4 c/p5 )";
    c.bench_function("depset-parse-dep", |b| {
        b.iter(|| {
            DependencySet::package(deps, &EAPI_LATEST_OFFICIAL).unwrap();
        })
    });

    c.bench_function("depset-restrict-dep", |b| {
        let dep: Dep = "c/p5".parse().unwrap();
        let r = Restrict::from(&dep);
        let depset = DependencySet::package(deps, &EAPI_LATEST_OFFICIAL).unwrap();
        b.iter(|| assert!(r.matches(&depset)));
    });
}
