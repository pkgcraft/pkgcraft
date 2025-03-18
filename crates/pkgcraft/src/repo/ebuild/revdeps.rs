use std::collections::{HashMap, HashSet};

use crate::dep::{ConditionalFlatten, Cpn, Cpv, Dep, UseDep};
use crate::pkg::ebuild::metadata::Key;
use crate::pkg::Package;
use crate::traits::LogErrors;

use super::EbuildRepo;

/// Reverse dependency entry for the RevDepCache.
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct RevDep {
    cpv: Cpv,
    use_deps: Vec<UseDep>,
    dep: Dep,
}

impl RevDep {
    /// Return the Cpv for the reverse dependency.
    pub fn cpv(&self) -> &Cpv {
        &self.cpv
    }

    /// Return the package dependency for the reverse dependency.
    pub fn dep(&self) -> &Dep {
        &self.dep
    }
}

/// Cache of reverse dependencies for an ebuild repo.
#[derive(Debug, Default)]
pub struct RevDepCache(HashMap<Cpn, HashMap<RevDep, HashSet<Key>>>);

impl RevDepCache {
    /// Create a reverse dependencies cache from an ebuild repo.
    pub fn from_repo(repo: &EbuildRepo, ignore: bool) -> crate::Result<Self> {
        let mut cache = Self::default();

        // TODO: build cache in parallel
        let mut iter = repo.iter_ordered().log_errors(ignore);
        for pkg in &mut iter {
            for key in pkg.eapi().dep_keys().iter().copied() {
                for (use_deps, dep) in pkg.dependencies([key]).into_iter_conditional_flatten()
                {
                    cache
                        .0
                        .entry(dep.cpn.clone())
                        .or_default()
                        .entry(RevDep {
                            cpv: pkg.cpv().clone(),
                            use_deps,
                            dep: dep.clone(),
                        })
                        .or_default()
                        .insert(key);
                }
            }
        }

        Ok(cache)
    }

    /// Get the reverse dependencies for a Cpn.
    pub fn get(&self, cpn: &Cpn) -> Option<&HashMap<RevDep, HashSet<Key>>> {
        self.0.get(cpn)
    }
}
