macro_rules! build_from_paths {
    ($base:expr, $($segment:expr),+) => {{
        let mut base: ::camino::Utf8PathBuf = $base.into();
        $(base.push($segment);)*
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
        crate::macros::assert_err_re!($res, $x, "");
    };
    ($res:expr, $re:expr, $msg:expr) => {
        let err = $res.unwrap_err();
        let s = err.to_string();
        let re = ::regex::Regex::new($re.as_ref()).unwrap();
        let err_msg = format!("{s:?} does not match regex: {:?}", $re);
        match $msg.is_empty() {
            true => assert!(re.is_match(&s), "{}", err_msg),
            false => assert!(re.is_match(&s), "{}", format!("{err_msg}: {}", $msg)),
        };
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

// Return Ordering if it's not equal.
macro_rules! cmp_not_equal {
    ($x:expr, $y:expr) => {
        let cmp = $x.cmp($y);
        if cmp != ::std::cmp::Ordering::Equal {
            return cmp;
        }
    };
}
pub(crate) use cmp_not_equal;
