macro_rules! build_from_paths {
    ($base:expr, $($segment:expr),+) => {{
        let mut base: ::camino::Utf8PathBuf = $base.into();
        $(base.push($segment);)*
        base
    }}
}
pub(crate) use build_from_paths;

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
            if lines.iter().any(|l| re.is_match(l)) {
                Ok(())
            } else {
                Err(format!("unmatched log regex: {re}"))
            }
        });
    };
}
#[cfg(test)]
pub(crate) use assert_logs_re;

// Return Ordering if the arguments or expression are not equal.
macro_rules! cmp_not_equal {
    ($cmp:expr) => {
        if $cmp != ::std::cmp::Ordering::Equal {
            return $cmp;
        }
    };
    ($x:expr, $y:expr) => {
        $crate::macros::cmp_not_equal!($x.cmp($y))
    };
}
pub(crate) use cmp_not_equal;

// Return False if the arguments or expression are not equal.
macro_rules! bool_not_equal {
    ($bool:expr) => {
        if !$bool {
            return $bool;
        }
    };
    ($x:expr, $y:expr) => {
        $crate::macros::bool_not_equal!($x.eq($y))
    };
}
pub(crate) use bool_not_equal;

// Hack to implement extend_left() support for VecDeque objects.
// TODO: extend_left() should be implemented upstream for VecDeque
macro_rules! extend_left {
    ($q:expr, $iter:expr) => {
        for item in $iter.rev() {
            $q.push_front(item);
        }
    };
}
pub(crate) use extend_left;
