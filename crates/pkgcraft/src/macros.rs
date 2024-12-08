/// Build a [`Utf8PathBuf`] path from a base and components.
#[macro_export]
macro_rules! build_path {
    ($base:expr, $($segment:expr),+) => {{
        let mut base: ::camino::Utf8PathBuf = $base.into();
        $(base.push($segment);)*
        base
    }}
}
pub use build_path;

// Return Ordering if the arguments or expression are not equal.
#[macro_export]
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

// Return false if the arguments are not equal or the expression is false.
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
