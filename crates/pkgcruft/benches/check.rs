use std::env;

use criterion::Criterion;
use pkgcraft::repo::Repo;
use pkgcruft::check::CheckKind;
use pkgcruft::scanner::Scanner;
use strum::IntoEnumIterator;

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("Check");
    group.sample_size(10);

    let path =
        env::var("PKGCRUFT_GENTOO_REPO").unwrap_or_else(|e| panic!("PKGCRUFT_GENTOO_REPO: {e}"));
    let repo = Repo::from_path("gentoo", path, 0, true).unwrap();
    // TODO: checkout a specific commit

    // run benchmark for every check
    for check in CheckKind::iter() {
        group.bench_function(check.to_string(), |b| {
            let scanner = Scanner::new().checks([check]);
            b.iter(|| scanner.run(&repo, [&repo]).count());
        });
    }
}
