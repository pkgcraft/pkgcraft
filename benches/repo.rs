use criterion::Criterion;

use pkgcraft::config::Config;
use pkgcraft::repo::Repository;
use pkgcraft::{atom, restrict::Restrict};

pub fn bench_repo_ebuild(c: &mut Criterion) {
    let mut config = Config::new("pkgcraft", "", false).unwrap();
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
        let r: Restrict = atom::cpv("cat/pkg-50").unwrap().into();
        b.iter(|| {
            pkgs = 0;
            for _ in repo.iter_restrict(r.clone()) {
                pkgs += 1;
            }
        });
        assert_eq!(pkgs, 1);
    });
}
