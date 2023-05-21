use std::ffi::c_char;

/// Convert a borrowed string to a raw C string.
///
/// Mostly used as a closure function along with iter_to_array.
pub(crate) fn str_to_raw<S: AsRef<str>>(s: S) -> *mut c_char {
    try_ptr_from_str!(s.as_ref())
}

/// Convert an iterator to a null-terminated array of object pointers using a given closure.
macro_rules! iter_to_array {
    ( $iter:expr, $func:expr ) => {{
        let mut ptrs: Vec<_> = $iter.map(|o| $func(o)).collect();
        ptrs.push(std::ptr::null_mut());
        ptrs.shrink_to_fit();
        ptrs
    }};
}
pub(crate) use iter_to_array;

/// Convert a given &str into a char*.
macro_rules! try_ptr_from_str {
    ( $s:expr ) => {{
        match std::ffi::CString::new($s) {
            Ok(s) => s.into_raw(),
            Err(e) => panic!("{e}"),
        }
    }};
}
pub(crate) use try_ptr_from_str;
