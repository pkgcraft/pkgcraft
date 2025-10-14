use std::cell::Cell;
use std::collections::HashMap;
use std::hash::Hash;
use std::process::ExitCode;
use std::rc::Rc;
use std::thread;

use camino::{Utf8Path, Utf8PathBuf};
use crossbeam_channel::{IntoIter, Receiver, Sender, bounded, unbounded};
use indexmap::{Equivalent, IndexSet};
use rayon::{iter::ParallelBridge, prelude::*};
use scallop::{ExecStatus, source};
use tracing::error;

use crate::utils::bounded_jobs;

/// Return true if a container type contains a given object, otherwise false.
pub trait Contains<T> {
    fn contains(&self, obj: T) -> bool;
}

impl<T, Q> Contains<&Q> for IndexSet<T>
where
    Q: Eq + Hash + Equivalent<T>,
    T: Eq + Hash,
{
    fn contains(&self, value: &Q) -> bool {
        IndexSet::contains(self, value)
    }
}

impl<T, V> Contains<&V> for &T
where
    T: for<'a> Contains<&'a V>,
{
    fn contains(&self, value: &V) -> bool {
        (*self).contains(value)
    }
}

/// Determine if two objects intersect.
pub trait Intersects<Rhs = Self> {
    fn intersects(&self, obj: &Rhs) -> bool;
}

/// Convert a borrowed type into an owned type.
pub trait IntoOwned {
    type Owned;
    fn into_owned(self) -> Self::Owned;
}

impl<T: IntoOwned> IntoOwned for Option<T> {
    type Owned = Option<T::Owned>;

    fn into_owned(self) -> Self::Owned {
        self.map(|x| x.into_owned())
    }
}

impl<T: IntoOwned> IntoOwned for Result<T, &crate::Error> {
    type Owned = Result<T::Owned, crate::Error>;

    fn into_owned(self) -> Self::Owned {
        self.map(|x| x.into_owned()).map_err(|e| e.clone())
    }
}

/// Create a borrowed type from an owned type.
pub trait ToRef<'a> {
    type Ref;
    fn to_ref(&'a self) -> Self::Ref;
}

impl<'a, T: ToRef<'a>> ToRef<'a> for Option<T> {
    type Ref = Option<T::Ref>;

    fn to_ref(&'a self) -> Self::Ref {
        self.as_ref().map(|x| x.to_ref())
    }
}

impl<'a, T: ToRef<'a>> ToRef<'a> for Result<T, crate::Error> {
    type Ref = Result<T::Ref, &'a crate::Error>;

    fn to_ref(&'a self) -> Self::Ref {
        self.as_ref().map(|x| x.to_ref())
    }
}

/// Iterate over an object's lines, filtering comments starting with '#' and empty lines returning
/// an enumerated iterator for the remaining content.
pub trait FilterLines {
    fn filter_lines(&self) -> impl Iterator<Item = (usize, &str)>;
}

impl<T: AsRef<str>> FilterLines for T {
    fn filter_lines(&self) -> impl Iterator<Item = (usize, &str)> {
        self.as_ref()
            .lines()
            .map(|s| s.trim())
            .enumerate()
            .map(|(i, s)| (i + 1, s))
            .filter(|(_, s)| !s.is_empty() && !s.starts_with('#'))
    }
}

/// Filter an iterable of results while conditionally logging errors.
pub trait LogErrors<I, T>
where
    I: IntoIterator<Item = crate::Result<T>>,
{
    fn log_errors(self, ignore: bool) -> LogErrorsIter<I::IntoIter, T>;
}

/// Iterator that filters an iterator of results while logging errors.
pub struct LogErrorsIter<I, T>
where
    I: Iterator<Item = crate::Result<T>>,
{
    iter: I,
    pub failed: Rc<Cell<bool>>,
    ignore: bool,
}

impl<I, T> From<LogErrorsIter<I, T>> for ExitCode
where
    I: Iterator<Item = crate::Result<T>>,
{
    fn from(iter: LogErrorsIter<I, T>) -> Self {
        ExitCode::from(iter.failed() as u8)
    }
}

impl<I, T> LogErrorsIter<I, T>
where
    I: Iterator<Item = crate::Result<T>>,
{
    /// Return true if any errors occurred during iteration, false otherwise.
    pub fn failed(&self) -> bool {
        self.failed.get()
    }
}

impl<I, T> Iterator for LogErrorsIter<I, T>
where
    I: Iterator<Item = crate::Result<T>>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        for result in &mut self.iter {
            match result {
                Ok(object) => return Some(object),
                Err(e) => {
                    if !self.ignore {
                        error!("{e}");
                        self.failed.set(true);
                    }
                    continue;
                }
            }
        }

        None
    }
}

impl<I, T> LogErrors<I, T> for I
where
    I: IntoIterator<Item = crate::Result<T>>,
{
    fn log_errors(self, ignore: bool) -> LogErrorsIter<I::IntoIter, T> {
        LogErrorsIter {
            iter: self.into_iter(),
            failed: Default::default(),
            ignore,
        }
    }
}

/// Convert the values of an iterable in parallel.
pub trait ParallelMap<I, F, T, R>
where
    I: IntoIterator<Item = T> + Send + 'static,
    F: Fn(T) -> R + Clone + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    fn par_map(self, func: F) -> ParallelMapBuilder<I, F, T, R>;
}

/// Builder for the ParallelMap trait.
pub struct ParallelMapBuilder<I, F, T, R>
where
    I: IntoIterator<Item = T> + Send + 'static,
    F: Fn(T) -> R + Clone + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    iterable: I,
    func: F,
    jobs: usize,
}

impl<I, F, T, R> ParallelMapBuilder<I, F, T, R>
where
    I: IntoIterator<Item = T> + Send + 'static,
    F: Fn(T) -> R + Clone + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    fn new(iterable: I, func: F) -> Self {
        Self {
            iterable,
            func,
            jobs: num_cpus::get(),
        }
    }

    /// Set the number of threads to use, defaults to all threads.
    pub fn jobs(mut self, value: usize) -> Self {
        self.jobs = bounded_jobs(value);
        self
    }
}

impl<I, F, T, R> IntoIterator for ParallelMapBuilder<I, F, T, R>
where
    I: IntoIterator<Item = T> + Send + 'static,
    <I as IntoIterator>::IntoIter: ParallelBridge + Send,
    F: Fn(T) -> R + Clone + Send + Sync + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    type Item = R;
    type IntoIter = ParallelMapIter<R>;

    fn into_iter(self) -> Self::IntoIter {
        ParallelMapIter::new(self.iterable, self.func, self.jobs)
    }
}

/// Iterator that converts values in parallel while retaining the original order.
pub struct ParallelMapIter<R: Send> {
    rx: Receiver<R>,
}

impl<R> ParallelMapIter<R>
where
    R: Send + 'static,
{
    fn new<I, F, T>(value: I, func: F, jobs: usize) -> Self
    where
        I: IntoIterator<Item = T> + Send + 'static,
        <I as IntoIterator>::IntoIter: ParallelBridge + Send,
        F: Fn(T) -> R + Clone + Send + Sync + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = bounded(jobs);

        rayon::spawn(move || {
            let _ = value
                .into_iter()
                .par_bridge()
                .try_for_each_with(tx, |tx, item| tx.send(func(item)));
        });

        Self { rx }
    }
}

impl<I, F, T, R> ParallelMap<I, F, T, R> for I
where
    I: IntoIterator<Item = T> + Send + 'static,
    F: Fn(T) -> R + Clone + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    fn par_map(self, func: F) -> ParallelMapBuilder<I, F, T, R> {
        ParallelMapBuilder::new(self, func)
    }
}

impl<R> Iterator for ParallelMapIter<R>
where
    R: Send + 'static,
{
    type Item = R;

    fn next(&mut self) -> Option<Self::Item> {
        match self.rx.recv() {
            Ok(item) => Some(item),
            Err(_) => None,
        }
    }
}

/// Convert the values of an iterable in parallel while retaining the original order.
pub trait ParallelMapOrdered<I, F, T, R>
where
    I: IntoIterator<Item = T> + Send + 'static,
    F: Fn(T) -> R + Clone + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    fn par_map_ordered(self, func: F) -> ParallelMapOrderedBuilder<I, F, T, R>;
}

impl<I, F, T, R> ParallelMapOrdered<I, F, T, R> for I
where
    I: IntoIterator<Item = T> + Send + 'static,
    F: Fn(T) -> R + Clone + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    fn par_map_ordered(self, func: F) -> ParallelMapOrderedBuilder<I, F, T, R> {
        ParallelMapOrderedBuilder::new(self, func)
    }
}

/// Builder for the ParallelMapOrdered trait.
pub struct ParallelMapOrderedBuilder<I, F, T, R>
where
    I: IntoIterator<Item = T> + Send + 'static,
    F: Fn(T) -> R + Clone + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    iterable: I,
    func: F,
    jobs: usize,
}

impl<I, F, T, R> ParallelMapOrderedBuilder<I, F, T, R>
where
    I: IntoIterator<Item = T> + Send + 'static,
    F: Fn(T) -> R + Clone + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    fn new(iterable: I, func: F) -> Self {
        Self {
            iterable,
            func,
            jobs: num_cpus::get(),
        }
    }

    /// Set the number of threads to use, defaults to all threads.
    pub fn jobs(mut self, value: usize) -> Self {
        self.jobs = bounded_jobs(value);
        self
    }
}

impl<I, F, T, R> IntoIterator for ParallelMapOrderedBuilder<I, F, T, R>
where
    I: IntoIterator<Item = T> + Send + 'static,
    F: Fn(T) -> R + Clone + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    type Item = R;
    type IntoIter = ParallelMapOrderedIter<R>;

    fn into_iter(self) -> Self::IntoIter {
        ParallelMapOrderedIter::new(self.iterable, self.func, self.jobs)
    }
}

/// Iterator that converts values in parallel while retaining the original order.
pub struct ParallelMapOrderedIter<R: Send> {
    threads: Vec<thread::JoinHandle<()>>,
    rx: Receiver<(usize, R)>,
    id: usize,
    cache: HashMap<usize, R>,
}

impl<R> ParallelMapOrderedIter<R>
where
    R: Send + 'static,
{
    fn new<I, F, T>(value: I, func: F, jobs: usize) -> Self
    where
        I: IntoIterator<Item = T> + Send + 'static,
        F: Fn(T) -> R + Clone + Send + 'static,
        T: Send + 'static,
    {
        let (input_tx, input_rx) = bounded(jobs);
        let (output_tx, output_rx) = bounded(jobs);
        let mut threads = vec![Self::producer(value, input_tx)];
        threads.extend(
            (0..jobs).map(|_| Self::worker(func.clone(), input_rx.clone(), output_tx.clone())),
        );

        Self {
            threads,
            rx: output_rx,
            id: 0,
            cache: Default::default(),
        }
    }

    fn producer<I, T>(value: I, tx: Sender<(usize, T)>) -> thread::JoinHandle<()>
    where
        I: IntoIterator<Item = T> + Send + 'static,
        T: Send + 'static,
    {
        thread::spawn(move || {
            for (id, item) in value.into_iter().enumerate() {
                tx.send((id, item)).ok();
            }
        })
    }

    fn worker<F, T>(
        func: F,
        rx: Receiver<(usize, T)>,
        tx: Sender<(usize, R)>,
    ) -> thread::JoinHandle<()>
    where
        F: Fn(T) -> R + Clone + Send + 'static,
        T: Send + 'static,
    {
        thread::spawn(move || {
            for (id, item) in rx {
                tx.send((id, func(item))).ok();
            }
        })
    }
}

impl<R> Iterator for ParallelMapOrderedIter<R>
where
    R: Send + 'static,
{
    type Item = R;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(value) = self.cache.remove(&self.id) {
                self.id += 1;
                return Some(value);
            } else if let Ok((id, value)) = self.rx.recv() {
                self.cache.insert(id, value);
                continue;
            } else {
                for thread in self.threads.drain(..) {
                    thread.join().unwrap();
                }
                return None;
            }
        }
    }
}

/// Support bash sourcing via file paths or directly from string content.
pub(crate) trait SourceBash {
    fn source_bash(&self) -> scallop::Result<ExecStatus>;
}

macro_rules! make_source_path_trait {
    ($($x:ty),+) => {$(
        impl SourceBash for $x {
            fn source_bash(&self) -> scallop::Result<ExecStatus> {
                if !self.exists() {
                    return Err(scallop::Error::Base(format!("nonexistent file: {self}")));
                }

                source::file(self)
            }
        }
    )+};
}
make_source_path_trait!(&Utf8Path, &Utf8PathBuf);

impl SourceBash for &str {
    fn source_bash(&self) -> scallop::Result<ExecStatus> {
        source::string(self)
    }
}
