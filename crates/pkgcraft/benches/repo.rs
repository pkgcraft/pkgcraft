use criterion::Criterion;

use pkgcraft::config::Config;
use pkgcraft::dep::Dep;
use pkgcraft::repo::PkgRepository;

pub fn bench_repo_ebuild(c: &mut Criterion) {
    let mut config = Config::new("pkgcraft", "");
    let (t, repo) = config.temp_repo("test", 0).unwrap();
    for i in 0..100 {
        t.create_ebuild(&format!("cat/pkg-{i}"), []).unwrap();
    }
    let repo = repo.as_ref();

    c.bench_function("repo-ebuild-iter", |b| {
        let mut pkgs = 0;
        b.iter(|| {
            pkgs = 0;
            for _ in repo {
                pkgs += 1;
            }
        });
        assert_eq!(pkgs, 100);
    });

    c.bench_function("repo-ebuild-iter-restrict", |b| {
        let mut pkgs = 0;
        let cpv = Dep::new_cpv("cat/pkg-50").unwrap();
        b.iter(|| {
            pkgs = 0;
            for _ in repo.iter_restrict(&cpv) {
                pkgs += 1;
            }
        });
        assert_eq!(pkgs, 1);
    });
}
