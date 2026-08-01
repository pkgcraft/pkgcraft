use criterion::Criterion;

use pkgcraft::config::Config;
use pkgcraft::repo::RepoFormat;
use pkgcraft::test::test_data;

pub fn bench_config(c: &mut Criterion) {
    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();

    c.bench_function("config-add-repo-path", |b| {
        let mut config = Config::new("pkgcraft", "");
        b.iter(|| config.add_repo_path("test", repo.path(), 0).is_ok());
        assert_eq!(config.repos().iter().count(), 1);
    });

    c.bench_function("config-add-format-repo-path", |b| {
        let mut config = Config::new("pkgcraft", "");
        b.iter(|| {
            config
                .add_format_repo_path("test", repo.path(), 0, RepoFormat::Ebuild)
                .is_ok()
        });
        assert_eq!(config.repos().iter().count(), 1);
    });

    c.bench_function("config-add-format-repo-path-wrong-format", |b| {
        let mut config = Config::new("pkgcraft", "");
        b.iter(|| {
            config
                .add_format_repo_path("test", repo.path(), 0, RepoFormat::Fake)
                .is_err()
        });
        assert_eq!(config.repos().iter().count(), 0);
    });

    c.bench_function("config-add-nested-repo-path", |b| {
        let mut config = Config::new("pkgcraft", "");
        let path = repo.path().join("profiles");
        b.iter(|| config.add_nested_repo_path(&path, 0).is_ok());
        assert_eq!(config.repos().iter().count(), 1);
    });

    c.bench_function("config-add-format-nested-repo-path", |b| {
        let mut config = Config::new("pkgcraft", "");
        let path = repo.path().join("profiles");
        b.iter(|| {
            config
                .add_format_repo_nested_path(&path, 0, RepoFormat::Ebuild)
                .is_ok()
        });
        assert_eq!(config.repos().iter().count(), 1);
    });
}
