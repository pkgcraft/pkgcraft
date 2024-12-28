use std::env;

use criterion::Criterion;
use pkgcraft::config::Config;
use pkgcruft::check::Check;
use pkgcruft::scan::Scanner;

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("Check");
    group.sample_size(10);

    if let Ok(path) = env::var("PKGCRUFT_BENCH_REPO") {
        let mut config = Config::new("pkgcraft", "");
        let repo = config
            .add_repo_path(&path, &path, 0, true)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();
        // TODO: checkout a specific commit

        // run benchmark for every check supported by the repo
        for check in Check::iter_supported(&repo) {
            group.bench_function(check.to_string(), |b| {
                let scanner = Scanner::new(&repo).checks([check]);
                b.iter(|| scanner.run(&repo).unwrap().count());
            });
        }
    } else {
        eprintln!("skipping check benchmarks: PKGCRUFT_BENCH_REPO unset");
    }
}
