use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;

use camino::Utf8Path;
use itertools::Itertools;
use rayon::prelude::*;

use crate::dep::{ConditionalFlatten, Cpn, Cpv, Dep, UseDep};
use crate::macros::build_path;
use crate::pkg::ebuild::metadata::Key;
use crate::pkg::Package;
use crate::traits::LogErrors;
use crate::Error;

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
        let mut iter = repo.iter_unordered().log_errors(ignore);
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

        if iter.failed() {
            Err(Error::InvalidValue("metadata failures occurred".to_string()))
        } else {
            Ok(cache)
        }
    }

    /// Get the reverse dependencies for a Cpn.
    pub fn get(&self, cpn: &Cpn) -> Option<&HashMap<RevDep, HashSet<Key>>> {
        self.0.get(cpn)
    }

    /// Serialize the cache to a directory using qa reports format.
    pub fn serialize_to_qa<P: AsRef<Utf8Path>>(&self, path: P) -> crate::Result<()> {
        let path = path.as_ref();
        let dir = path.join("revdeps");
        fs::create_dir_all(&dir)?;

        // convert cache into qa reports compatible mapping
        let mut mapping: HashMap<_, HashMap<_, Vec<_>>> = HashMap::new();
        for (cpn, revdeps) in &self.0 {
            for (revdep, keys) in revdeps {
                for key in keys {
                    mapping
                        .entry(key.as_ref().to_lowercase())
                        .or_default()
                        .entry(cpn)
                        .or_default()
                        .push(revdep);
                }
            }
        }

        // return the prefix for the USE dependency
        let enabled = |use_dep: &UseDep| -> &str {
            if use_dep.enabled() {
                ""
            } else {
                "!"
            }
        };

        // write entries to disk in the expected file layout and format
        for (key, revdeps) in mapping {
            revdeps.into_par_iter().try_for_each(
                |(cpn, mut revdeps)| -> crate::Result<()> {
                    let path = build_path!(&dir, &key, cpn.category(), cpn.package());
                    fs::create_dir_all(path.parent().unwrap())?;
                    let mut f = File::create(path)?;
                    revdeps.sort_by(|a, b| a.cpv.cmp(&b.cpv));
                    for r in revdeps {
                        let blocker = r.dep.blocker().map(|_| "[B]").unwrap_or_default();
                        let cpv = &r.cpv;
                        let flags = if !r.use_deps.is_empty() {
                            format!(
                                ":{}",
                                r.use_deps
                                    .iter()
                                    .sorted()
                                    .map(|x| format!("{}{}", enabled(x), x.flag()))
                                    .join("+")
                            )
                        } else {
                            Default::default()
                        };
                        writeln!(f, "{blocker}{cpv}{flags}")?;
                    }
                    Ok(())
                },
            )?;
        }

        Ok(())
    }
}
