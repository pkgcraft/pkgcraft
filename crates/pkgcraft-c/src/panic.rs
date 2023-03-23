use std::ptr;

// All function calls are wrapped in the ffi_catch_panic! macro that catches panics and
// early-returns from the function using the proper return value to signify an error as occurred.
// This trait provides that fallback value.
pub(crate) trait PanicOrDefault {
    fn value() -> Self;
}

// Defaultable is a subset of Default used as return values by pkgcraft-c.
pub(crate) trait Defaultable: Default {}

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
        // Override the default panic hook to suppress stderr output and restore it on completion.
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { $($tt)* }));
        std::panic::set_hook(prev_hook);

        match result {
            Ok(ret) => ret,
            Err(_) => return $crate::panic::PanicOrDefault::value(),
        }
    }
}
pub(crate) use ffi_catch_panic;
