use glob::glob;

use crate::report::{Iter, Report};

/// Return an iterator of reports from a globbed file path pattern.
pub fn glob_reports<P: AsRef<str>>(pattern: P) -> impl Iterator<Item = Report> {
    glob(pattern.as_ref())
        .unwrap()
        .filter_map(Result::ok)
        .flat_map(|path| {
            Iter::try_from_file(path, None)
                .unwrap()
                .filter_map(Result::ok)
        })
}
