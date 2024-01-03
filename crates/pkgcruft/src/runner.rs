use crossbeam_channel::Sender;
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
        tx: &Sender<Report>,
    ) -> crate::Result<()> {
        for item in self.source.iter_restrict(restrict) {
            for check in &self.checks {
                check.run(&item, tx)?;
            }
        }

        Ok(())
    }
}
