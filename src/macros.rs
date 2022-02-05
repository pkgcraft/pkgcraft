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
    ($res:expr, $x:expr) => {
        let err = $res.unwrap_err();
        let s = err.to_string();
        let re = ::regex::Regex::new($x.as_ref()).unwrap();
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

macro_rules! write_flush {
    ($handle:expr, $($t:tt)*) => {
        {
            let mut h = $handle;
            write!(h, $($t)* ).unwrap();
            h.flush().unwrap();
        }
    }
}
pub(crate) use write_flush;
