/// Panic on null pointer.
macro_rules! null_ptr_check {
    ( $ptr:expr ) => {
        match unsafe { $ptr } {
            Some(p) => p,
            None => panic!("unexpected null pointer argument"),
        }
    };
}
pub(crate) use null_ptr_check;

/// Unwrap an expression's Result or return a value.
macro_rules! unwrap_or_return {
    ( $e:expr, $v:expr ) => {
        match $e {
            Ok(x) => x,
            Err(e) => {
                $crate::error::update_last_error(e);
                return $v;
            }
        }
    };
}
pub(crate) use unwrap_or_return;

/// Return char or null pointer for Option wrapped strings.
macro_rules! char_p_or_null {
    ( $s:expr ) => {
        match $s {
            Some(s) => std::ffi::CString::new(s).unwrap().into_raw(),
            None => std::ptr::null_mut(),
        }
    };
}
pub(crate) use char_p_or_null;

/// Free char pointers or ignore null pointers.
macro_rules! char_p_or_null_free {
    ( $ptr:expr ) => {
        if !$ptr.is_null() {
            drop(std::ffi::CString::from_raw($ptr));
        }
    };
}
pub(crate) use char_p_or_null_free;

/// Convert an iterator to an array of object pointers using a given closure.
macro_rules! iter_to_array {
    ( $iter:expr, $len:expr, $func:expr ) => {{
        let mut ptrs: Vec<_> = $iter.map(|o| $func(o)).collect();
        ptrs.shrink_to_fit();
        unsafe { *$len = ptrs.len() };
        let ptr = ptrs.as_mut_ptr();
        std::mem::forget(ptrs);
        ptr
    }};
}
pub(crate) use iter_to_array;
