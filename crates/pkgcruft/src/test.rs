use glob::glob;

use crate::report::{Iter, Report};

/// Return an iterator of reports from a globbed file path pattern.
pub fn glob_reports<P: AsRef<str>>(pattern: P) -> impl Iterator<Item = Report> {
    glob(pattern.as_ref())
        .unwrap()
        .filter_map(Result::ok)
        .flat_map(|path| {
            let path = path
                .to_str()
                .unwrap_or_else(|| panic!("invalid path: {path:?}"));
            Iter::try_from_file(path, None, None)
                .unwrap()
                .filter_map(Result::ok)
        })
}
