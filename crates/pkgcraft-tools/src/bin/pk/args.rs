/// Apply bounds to use sane limits for parallel jobs.
pub(crate) fn bounded_jobs(jobs: Option<usize>) -> usize {
    let cpus = num_cpus::get();
    match jobs {
        Some(j) if j == 0 => 1,
        Some(j) if j <= cpus => j,
        _ => cpus,
    }
}
