#![cfg(test)]

/// Initialization for all test executables.
#[ctor::ctor]
fn initialize() {
    // initialize bash
    crate::shell::init(crate::shell::Env::default());
}

/// Assert an error matches a given regular expression for testing.
#[macro_export]
macro_rules! assert_err_re {
    ($res:expr, $x:expr) => {
        $crate::test::assert_err_re!($res, $x, "");
    };
    ($res:expr, $re:expr, $msg:expr) => {
        let err = $res.unwrap_err();
        let s = err.to_string();
        let re = ::regex::Regex::new($re.as_ref()).unwrap();
        let err_msg = format!("{s:?} does not match regex: {:?}", $re);
        if $msg.is_empty() {
            assert!(re.is_match(&s), "{}", err_msg);
        } else {
            assert!(re.is_match(&s), "{}", format!("{err_msg}: {}", $msg));
        }
    };
}
pub(crate) use assert_err_re;
