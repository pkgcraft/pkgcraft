use std::env;

use criterion::Criterion;
use pkgcraft::cli::Targets;
use pkgcraft::config::Config;
use pkgcraft::restrict::Scope;
use pkgcruft::check::Check;
use pkgcruft::scan::Scanner;

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("Check");
    group.sample_size(10);

    if let Ok(path) = env::var("PKGCRUFT_BENCH_REPO") {
        let mut config = Config::new("pkgcraft", "");
        let repo = Targets::new(&mut config)
            .finalize_repos([path])
            .unwrap()
            .ebuild_repo()
            .unwrap();

        // TODO: checkout a specific commit
        // run benchmark for every check supported by the repo
        let mut scanner = Scanner::new();
        for check in Check::iter_supported(&repo, Scope::Repo) {
            scanner = scanner.reports([check]);
            match scanner.run(&repo, &repo) {
                Ok(_) => {
                    group.bench_function(check.to_string(), |b| {
                        b.iter(|| scanner.run(&repo, &repo).unwrap().count());
                    });
                }
                Err(e) => eprintln!("skipping {check} check: {e}"),
            }
        }
    } else {
        eprintln!("skipping check benchmarks: PKGCRUFT_BENCH_REPO unset");
    }
}
