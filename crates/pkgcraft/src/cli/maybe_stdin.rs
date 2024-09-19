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

use std::io::{self, read_to_string, BufRead};
use std::str::FromStr;
use std::sync::atomic::AtomicBool;

use super::is_terminal;

pub(crate) static STDIN_HAS_BEEN_USED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, thiserror::Error)]
pub enum StdinError {
    #[error("stdin argument used more than once")]
    StdInRepeatedUse,
    #[error("stdin is a terminal")]
    StdinIsTerminal,
    #[error(transparent)]
    StdIn(#[from] io::Error),
    #[error("{0}")]
    FromStr(String),
}

/// Source of the value contents will be either from `stdin` or a CLI arg provided value
#[derive(Clone)]
pub enum Source {
    Stdin,
    Arg(String),
}

impl FromStr for Source {
    type Err = StdinError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "-" => {
                if STDIN_HAS_BEEN_USED.load(std::sync::atomic::Ordering::Acquire) {
                    return Err(StdinError::StdInRepeatedUse);
                }
                STDIN_HAS_BEEN_USED.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(Self::Stdin)
            }
            arg => Ok(Self::Arg(arg.to_owned())),
        }
    }
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::Stdin => write!(f, "stdin"),
            Source::Arg(v) => v.fmt(f),
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
    <T as FromStr>::Err: std::fmt::Display,
{
    type Err = StdinError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let source = Source::from_str(s)?;
        match &source {
            Source::Stdin => {
                let mut stdin = io::stdin().lock();
                if is_terminal!(&stdin) {
                    return Err(StdinError::StdinIsTerminal);
                }
                let input = read_to_string(&mut stdin)?;
                Ok(T::from_str(input.trim_end())
                    .map_err(|e| StdinError::FromStr(format!("{e}")))
                    .map(|val| Self { source, inner: val })?)
            }
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

impl<T> std::fmt::Display for MaybeStdin<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> std::fmt::Debug for MaybeStdin<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> std::ops::Deref for MaybeStdin<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> std::ops::DerefMut for MaybeStdin<T> {
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
    <T as FromStr>::Err: std::fmt::Display,
{
    type Err = StdinError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let source = Source::from_str(s)?;
        match &source {
            Source::Stdin => {
                let stdin = io::stdin().lock();
                if is_terminal!(&stdin) {
                    return Err(StdinError::StdinIsTerminal);
                }
                let mut inner = vec![];
                for arg in stdin.lines().map_while(Result::ok) {
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

impl<T> std::fmt::Debug for MaybeStdinVec<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> std::ops::Deref for MaybeStdinVec<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> std::ops::DerefMut for MaybeStdinVec<T> {
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
