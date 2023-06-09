/// Limit parallel jobs to the number of logical CPUs on a system. Note that this allows -j0 which
/// will hang pools using semaphores for access control.
pub(crate) fn bounded_jobs(jobs: Option<usize>) -> usize {
    let cpus = num_cpus::get();
    match jobs {
        Some(j) if j <= cpus => j,
        _ => cpus,
    }
}
