use criterion::Criterion;

use pkgcraft::restrict::Restrict;
use pkgcraft::test::TEST_DATA;
use pkgcruft::scanner::Scanner;

pub fn bench(c: &mut Criterion) {
    let repo = TEST_DATA.config().repos.get("qa-primary").unwrap();

    c.bench_function("scan", |b| {
        let scanner = Scanner::new();
        let restrict = Restrict::True;
        let mut count = 0;
        b.iter(|| {
            count = scanner.run(repo, [&restrict]).unwrap().count();
        });
        assert!(count > 0);
    });
}
