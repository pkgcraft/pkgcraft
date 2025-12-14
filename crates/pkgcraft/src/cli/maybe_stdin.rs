// This is clap-stdin (https://crates.io/crates/clap-stdin) with the addition of
// MaybeStdinVec to read stdin lines into a vector and other changes.
//
// Copyright (c) Matthew Wood
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::fmt;
use std::io::{self, read_to_string};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::io::stdin;

pub(crate) static STDIN_HAS_BEEN_USED: AtomicBool = AtomicBool::new(false);

#[derive(thiserror::Error, Debug)]
pub enum StdinError {
    #[error("stdin argument used more than once")]
    StdInRepeatedUse,
    #[error(transparent)]
    StdIn(#[from] io::Error),
    #[error("{0}")]
    FromStr(String),
}

/// Source of the value contents will be either from `stdin` or a CLI arg provided value
#[derive(Debug, Clone)]
pub enum Source {
    Stdin(String),
    Arg(String),
}

impl FromStr for Source {
    type Err = StdinError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "-" => {
                if STDIN_HAS_BEEN_USED.load(Ordering::Acquire) {
                    return Err(StdinError::StdInRepeatedUse);
                }
                let mut stdin = stdin();
                let input = read_to_string(&mut stdin)?;
                STDIN_HAS_BEEN_USED.store(true, Ordering::SeqCst);
                Ok(Self::Stdin(input))
            }
            arg => Ok(Self::Arg(arg.to_owned())),
        }
    }
}

/// Wrapper that parses arg values from `stdin`.
#[derive(Clone)]
pub struct MaybeStdin<T> {
    /// Source of the contents
    pub source: Source,
    inner: T,
}

impl<T> FromStr for MaybeStdin<T>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    type Err = StdinError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let source = Source::from_str(s)?;
        match &source {
            Source::Stdin(value) => Ok(T::from_str(value.trim_end())
                .map_err(|e| StdinError::FromStr(format!("{e}")))
                .map(|val| Self { source, inner: val })?),
            Source::Arg(value) => Ok(T::from_str(value)
                .map_err(|e| StdinError::FromStr(format!("{e}")))
                .map(|val| Self { source, inner: val })?),
        }
    }
}

impl<T> MaybeStdin<T> {
    /// Extract the inner value from the wrapper
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> fmt::Display for MaybeStdin<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> fmt::Debug for MaybeStdin<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> Deref for MaybeStdin<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for MaybeStdin<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Wrapper that parses arg values from `stdin` using lines as separate values.
#[derive(Clone)]
pub struct MaybeStdinVec<T> {
    /// Source of the contents
    pub source: Source,
    inner: Vec<T>,
}

impl<T> FromStr for MaybeStdinVec<T>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    type Err = StdinError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let source = Source::from_str(s)?;
        match &source {
            Source::Stdin(value) => {
                let mut inner = vec![];
                for arg in value.lines() {
                    let val = T::from_str(arg.trim_end())
                        .map_err(|e| StdinError::FromStr(format!("{e}")))?;
                    inner.push(val);
                }
                Ok(Self { source, inner })
            }
            Source::Arg(value) => Ok(T::from_str(value)
                .map_err(|e| StdinError::FromStr(format!("{e}")))
                .map(|val| Self { source, inner: vec![val] })?),
        }
    }
}

impl<T> MaybeStdinVec<T> {
    /// Extract the inner value from the wrapper
    pub fn into_inner(self) -> Vec<T> {
        self.inner
    }
}

impl<T> fmt::Debug for MaybeStdinVec<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> Deref for MaybeStdinVec<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for MaybeStdinVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T> IntoIterator for MaybeStdinVec<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a MaybeStdinVec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::test::assert_ordered_eq;

    use super::*;

    #[test]
    fn repeated_use() {
        let r: Result<MaybeStdin<String>, StdinError> = "-".parse();
        assert!(r.is_ok());
        let r: Result<MaybeStdin<String>, StdinError> = "-".parse();
        assert!(matches!(r, Err(StdinError::StdInRepeatedUse)));
    }

    #[test]
    fn maybe_stdin() {
        for use_stdin in [false, true] {
            let mut value: MaybeStdin<String> = if use_stdin {
                let mut stdin = stdin();
                stdin.write_all(b"test\n").unwrap();
                "-".parse().unwrap()
            } else {
                "test".parse().unwrap()
            };
            assert_eq!(value.to_string(), "test");
            assert!(format!("{value:?}").contains("test"));
            assert_eq!(value.len(), 4);
            value.push_str("test");
            assert_eq!(value.into_inner(), "testtest");
        }
    }

    #[test]
    fn maybe_stdin_vec() {
        for use_stdin in [false, true] {
            let mut values: MaybeStdinVec<usize> = if use_stdin {
                let mut stdin = stdin();
                stdin.write_all(b"12\n").unwrap();
                "-".parse().unwrap()
            } else {
                "12".parse().unwrap()
            };
            assert!(format!("{values:?}").contains("12"));
            assert_eq!(values.len(), 1);
            values.push(13);
            assert_ordered_eq!(values.clone().into_iter(), [12, 13]);
            assert_ordered_eq!((&values).into_iter(), [&12, &13]);
            assert_eq!(values.into_inner(), [12, 13]);
        }
    }
}
