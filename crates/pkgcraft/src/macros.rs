#[macro_export]
macro_rules! build_from_paths {
    ($base:expr, $($segment:expr),+) => {{
        let mut base: ::camino::Utf8PathBuf = $base.into();
        $(base.push($segment);)*
        base
    }}
}
pub use build_from_paths;

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
        if $msg.is_empty() {
            assert!(re.is_match(&s), "{}", err_msg);
        } else {
            assert!(re.is_match(&s), "{}", format!("{err_msg}: {}", $msg));
        }
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

// Return Ordering if the arguments or expression are not equal.
macro_rules! partial_cmp_not_equal {
    ($partial_cmp:expr) => {
        if let Some(cmp) = $partial_cmp {
            if cmp != ::std::cmp::Ordering::Equal {
                return cmp;
            }
        }
    };
    ($x:expr, $y:expr) => {
        $crate::macros::partial_cmp_not_equal!($x.partial_cmp($y))
    };
}
pub(crate) use partial_cmp_not_equal;

// Return Option<Ordering> if the arguments or expression are not equal.
macro_rules! partial_cmp_not_equal_opt {
    ($partial_cmp:expr) => {
        if let Some(cmp) = $partial_cmp {
            if cmp != ::std::cmp::Ordering::Equal {
                return Some(cmp);
            }
        }
    };
    ($x:expr, $y:expr) => {
        $crate::macros::partial_cmp_not_equal_opt!($x.partial_cmp($y))
    };
}
pub(crate) use partial_cmp_not_equal_opt;

// Return false if the arguments or expression are not equal.
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

// Return true if two Option-wrapped values are equal, false otherwise.
macro_rules! partial_eq_opt {
    ($x:expr, $y:expr) => {
        if let (Some(v1), Some(v2)) = ($x, $y) {
            v1 == v2
        } else {
            false
        }
    };
}
pub(crate) use partial_eq_opt;

// Return Ordering if two Option-wrapped arguments are not equal.
macro_rules! partial_cmp_opt_not_equal {
    ($x:expr, $y:expr) => {
        let cmp = match ($x, $y) {
            (Some(x), Some(y)) => x.partial_cmp(y),
            (Some(_), None) => Some(Ordering::Greater),
            (None, Some(_)) => Some(Ordering::Less),
            (None, None) => Some(Ordering::Equal),
        };
        if let Some(cmp) = cmp {
            if cmp != Ordering::Equal {
                return cmp;
            }
        }
    };
}
pub(crate) use partial_cmp_opt_not_equal;

// Return Option<Ordering> if two Option-wrapped arguments are not equal.
macro_rules! partial_cmp_opt_not_equal_opt {
    ($x:expr, $y:expr) => {
        let cmp = match ($x, $y) {
            (Some(x), Some(y)) => x.partial_cmp(y),
            (Some(_), None) => Some(Ordering::Greater),
            (None, Some(_)) => Some(Ordering::Less),
            (None, None) => Some(Ordering::Equal),
        };
        if let Some(cmp) = cmp {
            if cmp != Ordering::Equal {
                return Some(cmp);
            }
        }
    };
}
pub(crate) use partial_cmp_opt_not_equal_opt;

// Implement the Equivalent trait between owned and borrowed types.
macro_rules! equivalent {
    ($x:ty, $y:ty) => {
        impl indexmap::Equivalent<$x> for $y {
            fn equivalent(&self, key: &$x) -> bool {
                self == key
            }
        }

        impl indexmap::Equivalent<$y> for $x {
            fn equivalent(&self, key: &$y) -> bool {
                self == key
            }
        }
    };
}
pub(crate) use equivalent;
