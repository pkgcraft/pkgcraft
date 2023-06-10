/// Limit parallel jobs to the number of logical CPUs on a system.
pub(crate) fn bounded_jobs(jobs: Option<usize>) -> anyhow::Result<usize> {
    let cpus = num_cpus::get();
    match jobs {
        Some(j) if j == 0 => anyhow::bail!("jobs must be a positive integer"),
        Some(j) if j <= cpus => Ok(j),
        _ => Ok(cpus),
    }
}
