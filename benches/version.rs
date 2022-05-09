use std::str::FromStr;

use criterion::Criterion;

use pkgcraft::atom::Version;

#[allow(unused_must_use)]
pub fn bench_pkg_versions(c: &mut Criterion) {
    c.bench_function("version-parse", |b| b.iter(|| Version::from_str("1.2.3_alpha4-r5")));

    c.bench_function("version-cmp-eq", |b| {
        let v1 = Version::from_str("1.2.3").unwrap();
        let v2 = Version::from_str("1.2.3").unwrap();
        b.iter(|| v1 == v2);
    });

    c.bench_function("version-cmp-lt", |b| {
        let v1 = Version::from_str("1.2.3_alpha").unwrap();
        let v2 = Version::from_str("1.2.3").unwrap();
        b.iter(|| v1 < v2);
    });

    c.bench_function("version-cmp-gt", |b| {
        let v1 = Version::from_str("1.2.3_p").unwrap();
        let v2 = Version::from_str("1.2.3").unwrap();
        b.iter(|| v1 > v2);
    });

    c.bench_function("version-cmp-sort", |b| {
        let mut versions: Vec<_> = (0..100)
            .rev()
            .map(|s| Version::from_str(&format!("{}", s)).unwrap())
            .collect();
        b.iter(|| versions.sort());
    });
}
