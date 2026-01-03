#![cfg(test)]

/// Initialization for all test executables.
#[ctor::ctor]
fn initialize() {
    // initialize bash
    crate::shell::init(crate::shell::Env::new());
}

/// Assert an error matches a given regular expression for testing.
#[macro_export]
macro_rules! assert_err_re {
    ($result:expr, $pattern:expr) => {
        $crate::test::assert_err_re!($result, $pattern, "");
    };
    ($result:expr, $pattern:expr, $msg:expr) => {
        let err = $result.unwrap_err().to_string();
        let re = ::regex::Regex::new(&$pattern).unwrap();
        let err_msg = format!("{err:?} does not match regex: {re:?}");
        if $msg.is_empty() {
            assert!(re.is_match(&err), "{}", err_msg);
        } else {
            assert!(re.is_match(&err), "{}", format!("{err_msg}: {}", $msg));
        }
    };
}
pub(crate) use assert_err_re;
