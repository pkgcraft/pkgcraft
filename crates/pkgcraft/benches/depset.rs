use std::str::FromStr;

use criterion::Criterion;

use pkgcraft::dep::{self, Dep};
use pkgcraft::eapi::EAPI_LATEST_OFFICIAL;
use pkgcraft::restrict::{Restrict, Restriction};

pub fn bench_depsets(c: &mut Criterion) {
    c.bench_function("depset-parse-required-use", |b| {
        b.iter(|| {
            dep::parse::required_use_dep_set("u1? ( u2 )", &EAPI_LATEST_OFFICIAL).unwrap();
        })
    });

    let deps = "c/p1 u1? ( c/p2 !u2? ( c/p3 ) ) || ( c/p4 c/p5 )";
    c.bench_function("depset-parse-dep", |b| {
        b.iter(|| {
            dep::parse::dependencies_dep_set(deps, &EAPI_LATEST_OFFICIAL).unwrap();
        })
    });

    c.bench_function("depset-restrict-dep", |b| {
        let r = Restrict::from(&Dep::from_str("c/p5").unwrap());
        let depset = dep::parse::dependencies_dep_set(deps, &EAPI_LATEST_OFFICIAL).unwrap();
        b.iter(|| assert!(r.matches(&depset)));
    });
}
