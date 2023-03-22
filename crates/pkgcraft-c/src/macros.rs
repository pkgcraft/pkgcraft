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

/// Convert a given pointer into a &T.
macro_rules! try_ref_from_ptr {
    ( $var:ident ) => {
        match unsafe { $var.as_ref() } {
            Some(c) => c,
            None => {
                let e = $crate::error::Error::new("unexpected NULL reference");
                $crate::error::update_last_error(e);
                panic!()
            }
        }
    };
}
pub(crate) use try_ref_from_ptr;

/// Convert a given pointer into a &mut T.
macro_rules! try_mut_from_ptr {
    ( $var:ident ) => {
        match unsafe { $var.as_mut() } {
            Some(c) => c,
            None => {
                let e = $crate::error::Error::new("unexpected NULL reference");
                $crate::error::update_last_error(e);
                panic!()
            }
        }
    };
}
pub(crate) use try_mut_from_ptr;

/// Convert a given char* into a &str.
macro_rules! try_str_from_ptr {
    ( $var:ident ) => {{
        let p = $crate::macros::try_ref_from_ptr!($var);
        match unsafe { std::ffi::CStr::from_ptr(p).to_str() } {
            Ok(s) => s,
            Err(e) => {
                $crate::error::update_last_error(e);
                panic!()
            }
        }
    }};
}
pub(crate) use try_str_from_ptr;

/// Convert a given &str into a char*.
macro_rules! try_ptr_from_str {
    ( $s:expr ) => {{
        match std::ffi::CString::new($s) {
            Ok(s) => s.into_raw(),
            Err(e) => {
                $crate::error::update_last_error(e);
                panic!()
            }
        }
    }};
}
pub(crate) use try_ptr_from_str;

/// Unwrap an expression's Result or panic after registering the error.
macro_rules! unwrap_or_panic {
    ( $e:expr ) => {
        match $e {
            Ok(x) => x,
            Err(e) => {
                $crate::error::update_last_error(e);
                panic!()
            }
        }
    };
}
pub(crate) use unwrap_or_panic;
