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
        let mut ptrs: Vec<_> = $iter.map($func).collect();
        ptrs.shrink_to_fit();
        unsafe { *$len = ptrs.len() };
        let ptr = ptrs.as_mut_ptr();
        std::mem::forget(ptrs);
        ptr
    }};
}
pub(crate) use iter_to_array;

/// Convert a given pointer into a &T.
macro_rules! try_ref_from_ptr {
    ( $var:expr ) => {
        match unsafe { $var.as_ref() } {
            Some(c) => c,
            None => {
                let e = $crate::error::Error::new("unexpected NULL reference");
                $crate::macros::set_error_and_panic!(e);
            }
        }
    };
}
pub(crate) use try_ref_from_ptr;

/// Convert a given pointer into a &mut T.
macro_rules! try_mut_from_ptr {
    ( $var:expr ) => {
        match unsafe { $var.as_mut() } {
            Some(c) => c,
            None => {
                let e = $crate::error::Error::new("unexpected NULL reference");
                $crate::macros::set_error_and_panic!(e);
            }
        }
    };
}
pub(crate) use try_mut_from_ptr;

/// Convert a given char* into a &str.
macro_rules! try_str_from_ptr {
    ( $var:expr ) => {{
        let p = $crate::macros::try_ref_from_ptr!($var);
        match unsafe { std::ffi::CStr::from_ptr(p).to_str() } {
            Ok(s) => s,
            Err(e) => $crate::macros::set_error_and_panic!(e),
        }
    }};
}
pub(crate) use try_str_from_ptr;

/// Convert a given char* into an Option<&str>.
macro_rules! try_opt_str_from_ptr {
    ( $var:expr ) => {{
        unsafe {
            $var.as_ref()
                .map(|p| match std::ffi::CStr::from_ptr(p).to_str() {
                    Ok(s) => s,
                    Err(e) => $crate::macros::set_error_and_panic!(e),
                })
        }
    }};
}
pub(crate) use try_opt_str_from_ptr;

/// Convert a given &str into a char*.
macro_rules! try_ptr_from_str {
    ( $s:expr ) => {{
        match std::ffi::CString::new($s) {
            Ok(s) => s.into_raw(),
            Err(e) => $crate::macros::set_error_and_panic!(e),
        }
    }};
}
pub(crate) use try_ptr_from_str;

/// Unwrap an expression's Result or panic after registering the error.
macro_rules! unwrap_or_panic {
    ( $e:expr ) => {
        match $e {
            Ok(x) => x,
            Err(e) => $crate::macros::set_error_and_panic!(e),
        }
    };
}
pub(crate) use unwrap_or_panic;

/// Register a given error and then panic.
macro_rules! set_error_and_panic {
    ( $e:expr ) => {{
        $crate::error::update_last_error($e.clone());
        panic!("{}", $e)
    }};
}
pub(crate) use set_error_and_panic;
