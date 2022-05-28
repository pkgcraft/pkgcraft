macro_rules! build_from_paths {
    ($base:expr, $($segment:expr),+) => {{
        let mut base: ::std::path::PathBuf = $base.into();
        $(
            base.push($segment);
        )*
        base
    }}
}
pub(crate) use build_from_paths;

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
            ref e => panic!("expected `{}` but got `{e:?}`", stringify!($($pattern)+)),
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
        assert!(re.is_match(&s), "{s:?} does not match regex: {re}");
    };
}
#[cfg(test)]
pub(crate) use assert_err_re;

#[cfg(test)]
macro_rules! assert_logs_re {
    ($x:expr) => {
        let re = ::regex::Regex::new($x.as_ref()).unwrap();
        logs_assert(|lines: &[&str]| {
            let s = lines.join("\n");
            match re.is_match(&s) {
                false => Err(format!("{s:?} does not match regex: {re}")),
                true => Ok(()),
            }
        });
    };
}
#[cfg(test)]
pub(crate) use assert_logs_re;

// convert Vec<&str> to Vec<String>
macro_rules! vec_str {
    ($x:expr) => {
        $x.iter().map(|&s| s.to_string()).collect()
    };
}
pub(crate) use vec_str;
