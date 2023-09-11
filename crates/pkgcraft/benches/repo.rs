use criterion::Criterion;

use pkgcraft::config::Config;
use pkgcraft::dep::Cpv;
use pkgcraft::repo::PkgRepository;

pub fn bench_repo_ebuild(c: &mut Criterion) {
    let mut config = Config::new("pkgcraft", "");
    let t = config.temp_repo("test", 0, None).unwrap();
    for i in 0..100 {
        t.create_raw_pkg(&format!("cat/pkg-{i}"), &[]).unwrap();
    }
    let repo = t.repo();

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
        let cpv = Cpv::new("cat/pkg-50").unwrap();
        b.iter(|| {
            pkgs = 0;
            for _ in repo.iter_restrict(&cpv) {
                pkgs += 1;
            }
        });
        assert_eq!(pkgs, 1);
    });

    let t = config.temp_repo("regen-repo", 0, None).unwrap();
    for i in 0..10 {
        for j in 0..10 {
            t.create_raw_pkg(&format!("cat{i}/pkg-{j}"), &[]).unwrap();
        }
    }
    let repo = t.repo();

    c.bench_function("repo-ebuild-metadata-regen-force", |b| {
        b.iter(|| {
            let _ = repo.pkg_metadata_regen(None, true, false);
        });
    });

    c.bench_function("repo-ebuild-metadata-regen-verify", |b| {
        b.iter(|| {
            let _ = repo.pkg_metadata_regen(None, false, false);
        });
    });
}
