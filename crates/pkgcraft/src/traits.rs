use std::borrow::Borrow;

use camino::{Utf8Path, Utf8PathBuf};
use scallop::builtins::ExecStatus;
use scallop::source;

/// Iterate over an object's lines, filtering comments starting with '#' and empty lines returning
/// an enumerated iterator for the remaining content.
pub trait FilterLines {
    fn filter_lines(&self) -> Box<dyn Iterator<Item = (usize, &str)> + '_>;
}

impl<T: Borrow<str>> FilterLines for T {
    fn filter_lines(&self) -> Box<dyn Iterator<Item = (usize, &str)> + '_> {
        let iter = self
            .borrow()
            .lines()
            .map(|s| s.trim())
            .enumerate()
            .map(|(i, s)| (i + 1, s))
            .filter(|(_, s)| !s.is_empty() && !s.starts_with('#'));

        Box::new(iter)
    }
}

/// Support bash sourcing via file paths or directly from string content.
pub(crate) trait SourceBash {
    fn source_bash(self) -> scallop::Result<ExecStatus>;
}

macro_rules! make_source_path_trait {
    ($($x:ty),+) => {$(
        impl SourceBash for $x {
            fn source_bash(self) -> scallop::Result<ExecStatus> {
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
    fn source_bash(self) -> scallop::Result<ExecStatus> {
        source::string(self)
    }
}
