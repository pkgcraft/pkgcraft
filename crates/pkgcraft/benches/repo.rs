use criterion::Criterion;

use pkgcraft::config::Config;
use pkgcraft::dep::Cpv;
use pkgcraft::repo::PkgRepository;
use pkgcraft::test::TEST_DATA;

pub fn bench_repo_ebuild(c: &mut Criterion) {
    let mut config = Config::new("pkgcraft", "");
    let mut temp = config.temp_repo("test", 0, None).unwrap();
    let _pool = config.pool();
    for i in 0..100 {
        temp.create_raw_pkg(format!("cat/pkg-{i}"), &[]).unwrap();
    }
    let repo = temp.repo();

    c.bench_function("repo-ebuild-iter", |b| {
        let mut pkgs = 0;
        b.iter(|| {
            pkgs = 0;
            for _ in &repo {
                pkgs += 1;
            }
        });
        assert_eq!(pkgs, 100);
    });

    c.bench_function("repo-ebuild-iter-restrict", |b| {
        let mut pkgs = 0;
        let cpv = Cpv::try_new("cat/pkg-50").unwrap();
        b.iter(|| {
            pkgs = 0;
            for _ in repo.iter_restrict(&cpv) {
                pkgs += 1;
            }
        });
        assert_eq!(pkgs, 1);
    });

    let (_pool, repo) = TEST_DATA.ebuild_repo("metadata").unwrap();

    c.bench_function("repo-ebuild-metadata-regen-force", |b| {
        b.iter(|| {
            let _ = repo.metadata().cache().regen().force(true).run(repo);
        });
    });

    c.bench_function("repo-ebuild-metadata-regen-verify", |b| {
        b.iter(|| {
            let _ = repo.metadata().cache().regen().run(repo);
        });
    });
}
