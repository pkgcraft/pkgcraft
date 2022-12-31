use std::ffi::CString;

use bitflags::bitflags;

use crate::builtins::ExecStatus;
use crate::error::last_error;
use crate::{bash, Error};

bitflags! {
    /// Flag values used with source::string() for altering string evaluation.
    pub struct Eval: u32 {
        const NONE = 0;
        const NON_INTERACTIVE = bash::SEVAL_NONINT;
        const INTERACTIVE = bash::SEVAL_INTERACT;
        const NO_HISTORY = bash::SEVAL_NOHIST;
        const NO_FREE = bash::SEVAL_NOFREE;
        const RESET_LINE = bash::SEVAL_RESETLINE;
        const PARSE_ONLY = bash::SEVAL_PARSEONLY;
        const NO_LONGJMP = bash::SEVAL_NOLONGJMP;
        const FUNCDEF = bash::SEVAL_FUNCDEF;
        const ONE_COMMAND = bash::SEVAL_ONECMD;
        const NO_HISTORY_EXPANSION = bash::SEVAL_NOHISTEXP;
    }
}

#[cfg(feature = "plugin")]
pub fn string<S: AsRef<str>>(s: S) -> crate::Result<ExecStatus> {
    let s = s.as_ref();
    let file_str = CString::new("scallop::source::string").unwrap();
    let c_str = CString::new(s).unwrap();
    let str_ptr = c_str.as_ptr() as *mut _;
    // flush any previous error
    last_error();
    let ret = unsafe { bash::evalstring(str_ptr, file_str.as_ptr(), Eval::NO_FREE.bits() as i32) };
    let err = last_error();
    match ret {
        0 => Ok(ExecStatus::Success),
        _ => match err {
            Some(e) => Err(e),
            None => Err(Error::Base(format!("failed sourcing: {s}"))),
        },
    }
}

#[cfg(not(feature = "plugin"))]
pub fn string<S: AsRef<str>>(s: S) -> crate::Result<ExecStatus> {
    let s = s.as_ref();
    let c_str = CString::new(s).unwrap();
    let str_ptr = c_str.as_ptr() as *mut _;
    // flush any previous error
    last_error();
    let ret = unsafe { bash::scallop_evalstring(str_ptr, 0) };
    let err = last_error();
    match ret {
        0 => Ok(ExecStatus::Success),
        _ => match err {
            Some(e) => Err(e),
            None => Err(Error::Base(format!("failed sourcing: {s}"))),
        },
    }
}

#[cfg(feature = "plugin")]
pub fn file<S: AsRef<str>>(path: S) -> crate::Result<ExecStatus> {
    let path = path.as_ref();
    let c_str = CString::new(path).unwrap();
    // flush any previous error
    last_error();
    let ret = unsafe { bash::source_file(c_str.as_ptr(), 0) };
    let err = last_error();
    match ret {
        0 => Ok(ExecStatus::Success),
        _ => match err {
            Some(e) => Err(e),
            None => Err(Error::Base(format!("failed sourcing: {:?}", path))),
        },
    }
}

#[cfg(not(feature = "plugin"))]
pub fn file<S: AsRef<str>>(path: S) -> crate::Result<ExecStatus> {
    let path = path.as_ref();
    let c_str = CString::new(path).unwrap();
    // flush any previous error
    last_error();
    let ret = unsafe { bash::scallop_source_file(c_str.as_ptr()) };
    let err = last_error();
    match ret {
        0 => Ok(ExecStatus::Success),
        _ => match err {
            Some(e) => Err(e),
            None => Err(Error::Base(format!("failed sourcing: {:?}", path))),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use crate::source;
    use crate::variables::optional;

    #[test]
    fn test_source_string() {
        assert_eq!(optional("VAR"), None);

        source::string("VAR=1").unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");

        source::string("VAR=").unwrap();
        assert_eq!(optional("VAR").unwrap(), "");

        source::string("unset -v VAR").unwrap();
        assert_eq!(optional("VAR"), None);
    }

    #[test]
    fn test_source_string_error() {
        // bad bash code raises error
        let err = source::string("local VAR").unwrap_err();
        assert_eq!(err.to_string(), "local: can only be used in a function");

        // Sourcing continues when an error occurs because `set -e` isn't enabled.
        assert!(source::string("local VAR\nVAR=1").is_ok());
        assert_eq!(optional("VAR").unwrap(), "1");
    }

    #[test]
    fn test_source_file() {
        assert_eq!(optional("VAR"), None);
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        writeln!(file, "VAR=1").unwrap();
        source::file(&path).unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");

        writeln!(file, "VAR=").unwrap();
        source::file(&path).unwrap();
        assert_eq!(optional("VAR").unwrap(), "");

        writeln!(file, "unset -v VAR").unwrap();
        source::file(&path).unwrap();
        assert_eq!(optional("VAR"), None);
    }

    #[test]
    fn test_source_file_error() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        // bad bash code raises error
        writeln!(file, "local VAR").unwrap();
        let err = source::file(&path).unwrap_err();
        assert!(err
            .to_string()
            .ends_with("line 1: local: can only be used in a function"));

        // Sourcing continues when an error occurs because `set -e` isn't enabled.
        writeln!(file, "VAR=1").unwrap();
        assert!(source::file(&path).is_ok());
        assert_eq!(optional("VAR").unwrap(), "1");

        // nested source without `set -e`
        let mut file1 = NamedTempFile::new().unwrap();
        let mut file2 = NamedTempFile::new().unwrap();
        let path = file1.path().to_str().unwrap().to_string();
        writeln!(file1, "source {:?}", file2.path()).unwrap();
        writeln!(file2, "local VAR\nVAR=2").unwrap();
        assert!(source::file(path).is_ok());
        assert_eq!(optional("VAR").unwrap(), "2");
    }

    #[test]
    #[cfg(not(feature = "plugin"))]
    fn test_source_file_error_longjmp() {
        // enable immediate exit on error
        crate::builtins::set(&["-e"]).unwrap();

        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        // `set -e` causes immediate return
        writeln!(file, "local VAR\nVAR=1").unwrap();
        assert!(source::file(path).is_err());
        assert_eq!(optional("VAR"), None);

        // nested source with `set -e`
        let mut file1 = NamedTempFile::new().unwrap();
        let mut file2 = NamedTempFile::new().unwrap();
        let path = file1.path().to_str().unwrap().to_string();
        writeln!(file1, "source {:?}", file2.path()).unwrap();
        writeln!(file2, "local VAR\nVAR=2").unwrap();
        assert!(source::file(path).is_err());
        assert_eq!(optional("VAR"), None);
    }
}
