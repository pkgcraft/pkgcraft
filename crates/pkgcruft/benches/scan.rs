use criterion::Criterion;

use pkgcraft::test::TEST_DATA;
use pkgcruft::scanner::Scanner;

pub fn bench(c: &mut Criterion) {
    let (pool, repo) = TEST_DATA.repo("qa-primary").unwrap();

    c.bench_function("scan", |b| {
        let scanner = Scanner::new(&pool);
        let mut count = 0;
        b.iter(|| {
            count = scanner.run(repo, repo).count();
        });
        assert!(count > 0);
    });
}
