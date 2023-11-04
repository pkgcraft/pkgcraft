use std::str::FromStr;

use criterion::Criterion;

use pkgcraft::dep::{Intersects, Version};

pub fn bench_pkg_versions(c: &mut Criterion) {
    c.bench_function("version-parse", |b| b.iter(|| Version::from_str("1.2.3_alpha4-r5")));

    c.bench_function("version-cmp-eq", |b| {
        let v1 = Version::from_str("1.2.3a_beta4-r5").unwrap();
        let v2 = Version::from_str("1.2.3a_beta4-r5").unwrap();
        b.iter(|| v1 == v2);
    });

    c.bench_function("version-cmp-lt", |b| {
        let v1 = Version::from_str("1.2.3a_beta4-r4").unwrap();
        let v2 = Version::from_str("1.2.3a_beta5-r5").unwrap();
        b.iter(|| v1 < v2);
    });

    c.bench_function("version-intersects", |b| {
        let v1 = Version::from_str(">=1.2.3").unwrap();
        let v2 = Version::from_str("=1.2*").unwrap();
        b.iter(|| v1.intersects(&v2));
    });

    c.bench_function("version-cmp-sort-simple", |b| {
        let mut versions: Vec<_> = (0..100)
            .rev()
            .map(|s| Version::from_str(&format!("{}", s)).unwrap())
            .collect();
        b.iter(|| versions.sort());
    });

    c.bench_function("version-cmp-sort-complex", |b| {
        let mut versions: Vec<_> = [
            // major version
            "1.2.2b_beta2-r2",
            "2.1.1a_alpha1-r1",
            // minor version
            "2.1.1b_beta2-r2",
            "2.2.1a_alpha1-r1",
            // patch version
            "2.2.1b_beta2-r2",
            "2.2.2a_alpha1-r1",
            // letter suffix
            "2.2.2a_beta2-r2",
            "2.2.2b_alpha1-r1",
            // release suffix
            "2.2.2b_alpha2-r2",
            "2.2.2b_beta1-r1",
            // release suffix version
            "2.2.2b_beta1-r2",
            "2.2.2b_beta2-r1",
            // revision
            "2.2.2b_beta2-r1",
            "2.2.2b_beta2-r2",
        ]
        .into_iter()
        .rev()
        .map(|s| Version::from_str(s).unwrap())
        .collect();
        b.iter(|| versions.sort());
    });
}
