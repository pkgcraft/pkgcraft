use std::ptr;

// All function calls are wrapped in the ffi_catch_panic! macro that catches panics and
// early-returns from the function using the proper return value to signify an error as occurred.
// This trait provides that fallback value.
pub(crate) trait PanicOrDefault {
    fn value() -> Self;
}

// Defaultable is a subset of Default used as return values by pkgcraft-c.
pub(crate) trait Defaultable: Default {}

impl Defaultable for i32 {}
impl Defaultable for u64 {}
impl Defaultable for bool {}
impl Defaultable for () {}

impl<T: Defaultable> PanicOrDefault for T {
    fn value() -> Self {
        Default::default()
    }
}

impl<T> PanicOrDefault for *mut T {
    fn value() -> Self {
        ptr::null_mut()
    }
}

impl<T> PanicOrDefault for *const T {
    fn value() -> Self {
        ptr::null()
    }
}

macro_rules! ffi_catch_panic {
    ( $($tt:tt)* ) => {
        match std::panic::catch_unwind(|| {
            $($tt)*
        }) {
            Ok(ret) => ret,
            Err(_) => return $crate::panic::PanicOrDefault::value(),
        }
    }
}
pub(crate) use ffi_catch_panic;
