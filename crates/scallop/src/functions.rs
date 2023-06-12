use std::ffi::CString;

use crate::bash;
use crate::builtins::ExecStatus;
use crate::error::{ok_or_error, Error};
use crate::macros::*;

#[derive(Debug)]
pub struct Function<'a> {
    name: String,
    func: &'a mut bash::ShellVar,
}

impl Function<'_> {
    /// Execute a given shell function.
    pub fn execute(&mut self, args: &[&str]) -> crate::Result<ExecStatus> {
        let args = [&[self.name.as_str()], args].concat();
        let mut args = iter_to_array!(args.iter(), str_to_raw);
        ok_or_error(|| {
            let ret = unsafe {
                let words = bash::strvec_to_word_list(args.as_mut_ptr(), 0, 0);
                bash::scallop_execute_shell_function(self.func, words)
            };
            if ret == 0 {
                Ok(ExecStatus::Success)
            } else {
                Err(Error::Base(format!(
                    "failed running function: {}: exit status {}",
                    &self.name, ret
                )))
            }
        })
    }
}

/// Find a given shell function.
pub fn find<'a, S: AsRef<str>>(name: S) -> Option<Function<'a>> {
    let name = name.as_ref();
    let func_name = CString::new(name).unwrap();
    let func = unsafe { bash::find_function(func_name.as_ptr()).as_mut() };
    func.map(|f| Function {
        name: name.to_string(),
        func: f,
    })
}

/// Run a function in bash function scope.
pub fn bash_func<F>(name: &str, func: F) -> crate::Result<ExecStatus>
where
    F: FnOnce() -> crate::Result<ExecStatus>,
{
    let func_name = CString::new(name).unwrap();
    unsafe { bash::push_context(func_name.as_ptr() as *mut _, 0, bash::TEMPORARY_ENV) };
    let result = func();
    unsafe { bash::pop_context() };
    result
}

#[cfg(test)]
mod tests {
    use crate::builtins::local;
    use crate::source;
    use crate::variables::{bind, optional};

    use super::*;

    #[test]
    fn find_function() {
        assert!(find("foo").is_none());
        source::string("foo() { :; }").unwrap();
        assert!(find("foo").is_some());
    }

    #[test]
    fn execute_success() {
        assert_eq!(optional("VAR"), None);
        source::string("foo() { VAR=$1; }").unwrap();
        let mut func = find("foo").unwrap();
        func.execute(&[]).unwrap();
        assert_eq!(optional("VAR").unwrap(), "");
        func.execute(&["1"]).unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");
    }

    #[test]
    fn execute_failure() {
        source::string("foo() { nonexistent_cmd; }").unwrap();
        let mut func = find("foo").unwrap();
        assert!(func.execute(&[]).is_err());
    }

    #[test]
    fn bash_func_scope() {
        bind("VAR", "outer", None, None).unwrap();
        bash_func("func_name", || {
            let result = local(["VAR=inner"]);
            assert_eq!(optional("VAR").unwrap(), "inner");
            result
        })
        .unwrap();
        assert_eq!(optional("VAR").unwrap(), "outer");
    }
}
