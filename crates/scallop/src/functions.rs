use std::ffi::{c_char, CString};
use std::ptr;

use crate::bash;
use crate::builtins::ExecStatus;
use crate::error::ok_or_error;

#[derive(Debug)]
pub struct Function<'a> {
    name: String,
    func: &'a mut bash::ShellVar,
}

impl Function<'_> {
    /// Execute a given shell function.
    pub fn execute(&mut self, args: &[&str]) -> crate::Result<ExecStatus> {
        let args = [&[self.name.as_str()], args].concat();
        let arg_strs: Vec<CString> = args.iter().map(|s| CString::new(*s).unwrap()).collect();
        let mut arg_ptrs: Vec<*mut c_char> =
            arg_strs.iter().map(|s| s.as_ptr() as *mut _).collect();
        arg_ptrs.push(ptr::null_mut());
        let args = arg_ptrs.as_mut_ptr();
        ok_or_error(|| {
            unsafe {
                let words = bash::strvec_to_word_list(args, 0, 0);
                bash::scallop_execute_shell_function(self.func, words);
            }
        })
    }
}

/// Find a given shell function.
pub fn find<'a, S: AsRef<str>>(name: S) -> Option<Function<'a>> {
    let name = name.as_ref();
    let func_name = CString::new(name).unwrap();
    let func = unsafe { bash::find_function(func_name.as_ptr()).as_mut() };
    func.map(|f| Function { name: name.into(), func: f })
}

/// Run a function in bash function scope.
pub fn bash_func<S: AsRef<str>, F: FnOnce()>(name: S, func: F) {
    let func_name = CString::new(name.as_ref()).unwrap();
    unsafe { bash::push_context(func_name.as_ptr() as *mut _, 0, bash::TEMPORARY_ENV) };
    func();
    unsafe { bash::pop_context() };
}

#[cfg(test)]
mod tests {
    use crate::builtins::local;
    use crate::source;
    use crate::variables::{bind, optional};

    use super::*;

    #[test]
    fn test_find() {
        assert!(find("foo").is_none());
        source::string("foo() { :; }").unwrap();
        assert!(find("foo").is_some());
    }

    #[test]
    fn execute() {
        assert_eq!(optional("VAR"), None);
        source::string("foo() { VAR=$1; }").unwrap();
        let mut func = find("foo").unwrap();
        func.execute(&[]).unwrap();
        assert_eq!(optional("VAR").unwrap(), "");
        func.execute(&["1"]).unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");
    }

    #[test]
    fn test_bash_func() {
        bind("VAR", "outer", None, None).unwrap();
        bash_func("func_name", || {
            local(&["VAR=inner"]).unwrap();
            assert_eq!(optional("VAR").unwrap(), "inner");
        });
        assert_eq!(optional("VAR").unwrap(), "outer");
    }
}
