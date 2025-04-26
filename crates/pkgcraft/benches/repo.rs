use criterion::Criterion;

use pkgcraft::cli::Targets;
use pkgcraft::config::Config;
use pkgcraft::dep::Cpv;
use pkgcraft::repo::PkgRepository;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::test_data;

pub fn bench_repo_ebuild(c: &mut Criterion) {
    let mut config = Config::new("pkgcraft", "");
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    for i in 0..100 {
        temp.create_ebuild(format!("cat/pkg-{i}"), &[]).unwrap();
    }
    let repo = Targets::new(&mut config)
        .finalize_repos([temp.path()])
        .unwrap()
        .ebuild_repo()
        .unwrap();

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

    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();

    c.bench_function("repo-ebuild-metadata-regen-force", |b| {
        b.iter(|| {
            let _ = repo.metadata().cache().regen(repo).force(true).run();
        });
    });

    c.bench_function("repo-ebuild-metadata-regen-verify", |b| {
        b.iter(|| {
            let _ = repo.metadata().cache().regen(repo).run();
        });
    });
}
