use criterion::Criterion;

use pkgcraft::test::test_data;
use pkgcruft::scan::Scanner;

pub fn bench(c: &mut Criterion) {
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();

    c.bench_function("scan", |b| {
        let scanner = Scanner::new(repo);
        let mut count = 0;
        b.iter(|| {
            count = scanner.run(repo).unwrap().count();
        });
        assert!(count > 0);
    });
}
