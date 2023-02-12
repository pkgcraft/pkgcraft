use std::str::FromStr;

use criterion::Criterion;

use pkgcraft::dep::{self, PkgDep};
use pkgcraft::eapi::EAPI_LATEST;
use pkgcraft::restrict::{Restrict, Restriction};

pub fn bench_depsets(c: &mut Criterion) {
    c.bench_function("depset-parse-required-use", |b| {
        b.iter(|| {
            dep::parse::required_use("u1? ( u2 )", &EAPI_LATEST).unwrap();
        })
    });

    let deps = "c/p1 u1? ( c/p2 !u2? ( c/p3 ) ) || ( c/p4 c/p5 )";
    c.bench_function("depset-parse-pkgdep", |b| {
        b.iter(|| {
            dep::parse::dependencies(deps, &EAPI_LATEST).unwrap();
        })
    });

    c.bench_function("depset-restrict-pkgdep", |b| {
        let r = Restrict::from(&PkgDep::from_str("c/p5").unwrap());
        let depset = dep::parse::dependencies(deps, &EAPI_LATEST)
            .unwrap()
            .unwrap();
        b.iter(|| assert!(r.matches(&depset)));
    });
}
