use std::ffi::CString;

use bitflags::bitflags;

use crate::error::{Error, ok_or_error};
use crate::{ExecStatus, bash};

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

pub fn string<S: AsRef<str>>(s: S) -> crate::Result<ExecStatus> {
    let c_str = CString::new(s.as_ref()).unwrap();
    ok_or_error(|| unsafe {
        let flags = Eval::RESET_LINE.bits() as i32;
        let ret = bash::scallop_evalstring(c_str.as_ptr(), flags);
        if ret == 0 {
            Ok(ExecStatus::Success)
        } else {
            Err(Error::Base(format!("failed sourcing string: exit status {}", ret)))
        }
    })
}

pub fn file<S: AsRef<str>>(path: S) -> crate::Result<ExecStatus> {
    let path = path.as_ref();
    let c_str = CString::new(path).unwrap();
    ok_or_error(|| unsafe {
        let ret = bash::scallop_source_file(c_str.as_ptr());
        if ret == 0 {
            Ok(ExecStatus::Success)
        } else {
            Err(Error::Base(format!("failed sourcing file: {path}: exit status {}", ret)))
        }
    })
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
        assert_eq!(err.to_string(), "line 1: local: can only be used in a function");

        // Sourcing continues when an error occurs because `set -e` isn't enabled, but the error is
        // still raised on completion (unlike bash).
        let err = source::string("local VAR\nVAR=1").unwrap_err();
        assert_eq!(optional("VAR").unwrap(), "1");
        assert_eq!(err.to_string(), "line 1: local: can only be used in a function");
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
        assert_eq!(
            err.to_string(),
            format!("{path}: line 1: local: can only be used in a function")
        );

        // Sourcing continues when an error occurs because `set -e` isn't enabled, but the error is
        // still raised on completion (unlike bash).
        writeln!(file, "VAR=0").unwrap();
        let err = source::file(&path).unwrap_err();
        assert_eq!(optional("VAR").unwrap(), "0");
        assert_eq!(
            err.to_string(),
            format!("{path}: line 1: local: can only be used in a function")
        );

        // nested source without `set -e`
        let mut file1 = NamedTempFile::new().unwrap();
        let mut file2 = NamedTempFile::new().unwrap();
        let file1_path = file1.path().to_str().unwrap().to_string();
        let file2_path = file2.path().to_str().unwrap().to_string();
        writeln!(file1, "source {:?}", file2_path).unwrap();
        writeln!(file2, "VAR=1\nlocal VAR\nVAR=2").unwrap();
        let err = source::file(file1_path).unwrap_err();
        assert_eq!(optional("VAR").unwrap(), "2");
        assert_eq!(
            err.to_string(),
            format!("{file2_path}: line 2: local: can only be used in a function")
        );
    }

    #[test]
    fn test_source_file_error_longjmp() {
        // enable immediate exit on error
        crate::builtins::set(["-e"]).unwrap();

        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        // `set -e` causes immediate return
        writeln!(file, "local VAR\nVAR=1").unwrap();
        assert!(source::file(path).is_err());
        assert_eq!(optional("VAR"), None);

        // nested source with `set -e`
        let mut file1 = NamedTempFile::new().unwrap();
        let mut file2 = NamedTempFile::new().unwrap();
        let file1_path = file1.path().to_str().unwrap().to_string();
        let file2_path = file2.path().to_str().unwrap().to_string();
        writeln!(file1, "source {file2_path}").unwrap();
        writeln!(file2, "VAR=1\nlocal VAR\nVAR=2").unwrap();
        let err = source::file(file1_path).unwrap_err();
        assert_eq!(optional("VAR").unwrap(), "1");
        assert_eq!(
            err.to_string(),
            format!("{file2_path}: line 2: local: can only be used in a function")
        );
    }
}
