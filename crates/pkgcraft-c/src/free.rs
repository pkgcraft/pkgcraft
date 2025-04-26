use std::ffi::{c_char, c_void, CString};

/// Free a string.
///
/// # Safety
/// The argument must be a string pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_str_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}

/// Free an array of strings.
///
/// # Safety
/// The argument must be a pointer to a string array or NULL along with the length of the array.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_str_array_free(strs: *mut *mut c_char, len: usize) {
    if !strs.is_null() {
        unsafe {
            for s in Vec::from_raw_parts(strs, len, len) {
                drop(CString::from_raw(s));
            }
        }
    }
}

/// Free an array without dropping the objects inside it.
///
/// # Safety
/// The array objects should be explicitly dropped using other methods otherwise they will leak.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_array_free(array: *mut *mut c_void, len: usize) {
    if !array.is_null() {
        unsafe { Vec::from_raw_parts(array, len, len) };
    }
}
