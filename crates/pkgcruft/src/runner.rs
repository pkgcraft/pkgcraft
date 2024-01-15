use pkgcraft::restrict::Restrict;

use crate::check::CheckRun;
use crate::report::Report;
use crate::source::IterRestrict;

#[derive(Debug)]
pub(crate) struct CheckRunner<C, S, T>
where
    C: CheckRun<T>,
    S: IterRestrict<Item = T>,
{
    checks: Vec<C>,
    source: S,
}

impl<C, S, T> Clone for CheckRunner<C, S, T>
where
    C: CheckRun<T> + Clone,
    S: IterRestrict<Item = T> + Clone,
{
    fn clone(&self) -> Self {
        Self {
            checks: self.checks.clone(),
            source: self.source.clone(),
        }
    }
}

impl<C, S, T> CheckRunner<C, S, T>
where
    C: CheckRun<T>,
    S: IterRestrict<Item = T>,
{
    pub(crate) fn new(source: S) -> Self {
        Self {
            checks: Default::default(),
            source,
        }
    }

    pub(crate) fn push(&mut self, check: C) {
        self.checks.push(check);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }

    pub(crate) fn run<R: Into<Restrict>>(
        &self,
        restrict: R,
        reports: &mut Vec<Report>,
    ) -> crate::Result<()> {
        for item in self.source.iter_restrict(restrict) {
            for check in &self.checks {
                check.run(&item, reports)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct CheckRunnerSet<C1, C2, S, T>
where
    C1: CheckRun<T>,
    C2: CheckRun<Vec<T>>,
    S: IterRestrict<Item = T>,
{
    pub(crate) item_checks: Vec<C1>,
    pub(crate) set_checks: Vec<C2>,
    source: S,
}

impl<C1, C2, S, T> Clone for CheckRunnerSet<C1, C2, S, T>
where
    C1: CheckRun<T> + Clone,
    C2: CheckRun<Vec<T>> + Clone,
    S: IterRestrict<Item = T> + Clone,
{
    fn clone(&self) -> Self {
        Self {
            item_checks: self.item_checks.clone(),
            set_checks: self.set_checks.clone(),
            source: self.source.clone(),
        }
    }
}

impl<C1, C2, S, T> CheckRunnerSet<C1, C2, S, T>
where
    C1: CheckRun<T>,
    C2: CheckRun<Vec<T>>,
    S: IterRestrict<Item = T>,
{
    pub(crate) fn new(source: S) -> Self {
        Self {
            item_checks: Default::default(),
            set_checks: Default::default(),
            source,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.item_checks.is_empty() && self.set_checks.is_empty()
    }

    pub(crate) fn run<R: Into<Restrict>>(
        &self,
        restrict: R,
        reports: &mut Vec<Report>,
    ) -> crate::Result<()> {
        let mut items = vec![];

        for item in self.source.iter_restrict(restrict) {
            for check in &self.item_checks {
                check.run(&item, reports)?;
            }
            items.push(item);
        }

        if !items.is_empty() {
            for check in &self.set_checks {
                check.run(&items, reports)?;
            }
        }

        Ok(())
    }
}
