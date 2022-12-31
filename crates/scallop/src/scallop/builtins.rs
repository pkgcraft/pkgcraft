use std::mem;

use crate::bash;
use crate::builtins::Builtin;

/// Register builtins into the internal list for use.
pub fn register(builtins: &[Builtin]) {
    unsafe {
        // convert builtins into pointers
        let mut builtin_ptrs: Vec<_> = builtins
            .iter()
            .map(|b| Box::into_raw(Box::new((*b).into())))
            .collect();

        // add builtins to bash's internal list
        bash::register_builtins(builtin_ptrs.as_mut_ptr(), builtin_ptrs.len());

        // reclaim pointers for proper deallocation
        for b in builtin_ptrs {
            mem::drop(Box::from_raw(b));
        }
    }
}
