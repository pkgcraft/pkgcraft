// convert &str to Option<String>
#[cfg(test)]
macro_rules! opt_str {
    ($x:expr) => {
        Some($x.to_string())
    };
}
#[cfg(test)]
pub(crate) use opt_str;

#[cfg(test)]
macro_rules! assert_err {
    ($expression:expr, $($pattern:tt)+) => {
        match $expression {
            $($pattern)+ => (),
            ref e => panic!("expected `{}` but got `{:?}`", stringify!($($pattern)+), e),
        }
    }
}
#[cfg(test)]
pub(crate) use assert_err;

#[cfg(test)]
macro_rules! assert_err_re {
    ($err:expr, $x:expr) => {
        let s = $err.to_string();
        let re = Regex::new($x).unwrap();
        assert!(re.is_match(&s), "{:?} does not match regex: {}", s, re);
    };
}
#[cfg(test)]
pub(crate) use assert_err_re;

// convert Vec<&str> to Vec<String>
macro_rules! vec_str {
    ($x:expr) => {
        $x.iter().map(|&s| s.to_string()).collect()
    };
}
pub(crate) use vec_str;
