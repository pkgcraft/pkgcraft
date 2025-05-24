use git2::Diff;
use indexmap::IndexSet;
use pkgcraft::dep::Cpn;

use crate::Result;

/// Determine targeted Cpns from a git diff.
pub fn diff_to_cpns(_diff: &Diff<'_>) -> Result<IndexSet<Cpn>> {
    // TODO: extract package restrictions from pushed changes
    Ok(IndexSet::new())
}
