use std::ffi::{CStr, CString};
use std::slice;

use bitflags::bitflags;

use crate::builtins::ExecStatus;
use crate::error::ok_or_error;
use crate::traits::*;
use crate::{bash, Error};

bitflags! {
    /// Flags for various attributes a given variable can have.
    pub struct Attr: u32 {
        const NONE = 0;
        const EXPORTED = bash::att_exported;
        const READONLY = bash::att_readonly;
        const ARRAY = bash::att_array;
        const FUNCTION = bash::att_function;
        const INTEGER = bash::att_integer;
        const LOCAL = bash::att_local;
        const ASSOC = bash::att_assoc;
        const TRACE = bash::att_trace;
        const UPPERCASE = bash::att_uppercase;
        const LOWERCASE = bash::att_lowercase;
        const CAPCASE = bash::att_capcase;
        const NAMEREF = bash::att_nameref;
        const INVISIBLE = bash::att_invisible;
        const NO_UNSET = bash::att_nounset;
        const NO_ASSIGN = bash::att_noassign;
    }
}

bitflags! {
    /// Flag values controlling how assignment statements are treated.
    pub struct Assign: u32 {
        const NONE = 0;
        const APPEND = bash::ASS_APPEND;
        const LOCAL = bash::ASS_MKLOCAL;
        const ASSOC = bash::ASS_MKASSOC;
        const GLOBAL = bash::ASS_MKGLOBAL;
        const NAMEREF = bash::ASS_NAMEREF;
        const FORCE = bash::ASS_FORCE;
        const CHKLOCAL = bash::ASS_CHKLOCAL;
        const NOEXPAND = bash::ASS_NOEXPAND;
        const NOEVAL = bash::ASS_NOEVAL;
        const NOLONGJMP = bash::ASS_NOLONGJMP;
        const NOINVIS = bash::ASS_NOINVIS;
    }
}

/// Unset a given variable name ignoring if it is nonexistent.
pub fn unbind<S: AsRef<str>>(name: S) -> crate::Result<ExecStatus> {
    let name = name.as_ref();
    let cstr = CString::new(name).unwrap();
    ok_or_error(|| {
        // ignore non-zero return values for nonexistent variables
        unsafe { bash::check_unbind_variable(cstr.as_ptr()) };
        Ok(ExecStatus::Success)
    })
}

pub fn bind<S1, S2>(
    name: S1,
    value: S2,
    flags: Option<Assign>,
    attrs: Option<Attr>,
) -> crate::Result<ExecStatus>
where
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    let name = CString::new(name.as_ref()).unwrap();
    let value = CString::new(value.as_ref()).unwrap();
    let val = value.as_ptr() as *mut _;
    let flags = flags.unwrap_or(Assign::NONE).bits() as i32;
    ok_or_error(|| {
        let var = unsafe { bash::bind_variable(name.as_ptr(), val, flags).as_mut() };
        if let Some(var) = var {
            if let Some(attrs) = attrs {
                var.attributes |= attrs.bits() as i32;
            }
        }
        Ok(ExecStatus::Success)
    })
}

pub fn bind_global<S: AsRef<str>>(
    name: S,
    value: S,
    flags: Option<Assign>,
    attrs: Option<Attr>,
) -> crate::Result<ExecStatus> {
    let name = CString::new(name.as_ref()).unwrap();
    let value = CString::new(value.as_ref()).unwrap();
    let val = value.as_ptr() as *mut _;
    let flags = flags.unwrap_or(Assign::NONE).bits() as i32;
    ok_or_error(|| {
        let var = unsafe { bash::bind_global_variable(name.as_ptr(), val, flags).as_mut() };
        if let Some(var) = var {
            if let Some(attrs) = attrs {
                var.attributes |= attrs.bits() as i32;
            }
        }
        Ok(ExecStatus::Success)
    })
}

#[derive(Debug, Clone)]
pub struct Variable {
    name: String,
}

impl Variable {
    pub fn new<S: Into<String>>(name: S) -> Self {
        Variable { name: name.into() }
    }
}

pub trait Variables: AsRef<str> {
    fn name(&self) -> &str;

    fn optional(&self) -> Option<String> {
        optional(self.name())
    }

    fn required(&self) -> crate::Result<String> {
        required(self.name())
    }

    fn expand(&self) -> Option<String> {
        self.optional().and_then(expand)
    }

    fn string_vec(&self) -> Option<Vec<String>> {
        string_vec(self.name())
    }

    fn bind<S: AsRef<str>>(
        &mut self,
        value: S,
        flags: Option<Assign>,
        attrs: Option<Attr>,
    ) -> crate::Result<ExecStatus> {
        bind(self.name(), value.as_ref(), flags, attrs)
    }

    fn bind_global<S: AsRef<str>>(
        &mut self,
        value: S,
        flags: Option<Assign>,
        attrs: Option<Attr>,
    ) -> crate::Result<ExecStatus> {
        bind_global(self.name(), value.as_ref(), flags, attrs)
    }

    fn unbind(&mut self) -> crate::Result<ExecStatus> {
        unbind(self.name())
    }

    fn append(&mut self, s: &str) -> crate::Result<ExecStatus> {
        self.bind(s, Some(Assign::APPEND), None)
    }

    fn shell_var(&self) -> Option<&mut bash::ShellVar> {
        let var_name = CString::new(self.name()).unwrap();
        unsafe { bash::find_variable(var_name.as_ptr()).as_mut() }
    }

    fn is_array(&self) -> bool {
        match self.shell_var() {
            None => false,
            Some(v) => v.attributes as u32 & Attr::ARRAY.bits() != 0,
        }
    }

    fn is_readonly(&self) -> bool {
        match self.shell_var() {
            None => false,
            Some(v) => v.attributes as u32 & Attr::READONLY.bits() != 0,
        }
    }
}

impl AsRef<str> for Variable {
    fn as_ref(&self) -> &str {
        self.name()
    }
}

impl Variables for Variable {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub struct ScopedVariable {
    var: Variable,
    orig: Option<String>,
}

/// Variable that will reset itself to its original value when it leaves scope.
impl ScopedVariable {
    pub fn new<S: Into<String>>(name: S) -> Self {
        let var = Variable::new(name);
        let orig = optional(&var.name);
        ScopedVariable { var, orig }
    }
}

impl Variables for ScopedVariable {
    fn name(&self) -> &str {
        &self.var.name
    }
}

impl AsRef<str> for ScopedVariable {
    fn as_ref(&self) -> &str {
        self.name()
    }
}

impl Drop for ScopedVariable {
    fn drop(&mut self) {
        if optional(&self.var.name) != self.orig {
            let mut reset = || -> crate::Result<ExecStatus> {
                if let Some(val) = &self.orig {
                    self.var.bind(val, None, None)
                } else {
                    self.var.unbind()
                }
            };
            reset().unwrap_or_else(|_| panic!("failed resetting variable: {}", self.var.name));
        }
    }
}

/// Get the raw string value of a given variable name, returning None when nonexistent.
pub fn optional<S: AsRef<str>>(name: S) -> Option<String> {
    let name = CString::new(name.as_ref()).unwrap();
    let ptr = unsafe { bash::get_string_value(name.as_ptr()).as_ref() };
    ptr.map(|s| unsafe { String::from(CStr::from_ptr(s).to_str().unwrap()) })
}

/// Get the raw string value of a given variable name, returning an Error when nonexistent.
pub fn required<S: AsRef<str>>(name: S) -> crate::Result<String> {
    let name = name.as_ref();
    optional(name).ok_or_else(|| Error::Base(format!("undefined variable: {name}")))
}

/// Get the expanded value of a given string.
pub fn expand<S: AsRef<str>>(val: S) -> Option<String> {
    let val = CString::new(val.as_ref()).unwrap();
    let ptr = unsafe { bash::expand_string_to_string(val.as_ptr() as *mut _, 0).as_ref() };
    ptr.map(|s| unsafe { String::from(CStr::from_ptr(s).to_str().unwrap()) })
}

/// Get the string value of a given variable name splitting it into Vec<String> based on IFS.
pub fn string_vec<S: AsRef<str>>(name: S) -> Option<Vec<String>> {
    let name = name.as_ref();
    let var_name = CString::new(name).unwrap();
    unsafe {
        bash::get_string_value(var_name.as_ptr()).as_mut().map(|s| {
            bash::list_string(s, bash::IFS, 1)
                .into_words(true)
                .into_iter()
                .map(|s| s.to_string())
                .collect()
        })
    }
}

/// Get the value of an array for a given variable name.
pub fn array_to_vec<S: AsRef<str>>(name: S) -> crate::Result<Vec<String>> {
    let name = name.as_ref();
    let var_name = CString::new(name).unwrap();
    let var = unsafe { bash::find_variable(var_name.as_ptr()).as_ref() };
    let array_ptr = match var {
        None => Err(Error::Base(format!("undefined variable: {name}"))),
        Some(v) => {
            if (v.attributes as u32 & Attr::ARRAY.bits()) != 0 {
                Ok(v.value as *mut bash::Array)
            } else {
                Err(Error::Base(format!("variable is not an array: {name}")))
            }
        }
    }?;

    let mut count: i32 = 0;
    let strings: Vec<String>;

    unsafe {
        let str_array = bash::array_to_argv(array_ptr, &mut count);
        strings = slice::from_raw_parts(str_array, count as usize)
            .iter()
            .map(|s| String::from(CStr::from_ptr(*s).to_str().unwrap()))
            .collect();
        bash::strvec_dispose(str_array);
    }

    Ok(strings)
}

/// Get the value of a given variable as Vec<String>.
pub fn var_to_vec<S: AsRef<str>>(name: S) -> crate::Result<Vec<String>> {
    let name = name.as_ref();
    let var = Variable::new(name);
    if var.is_array() {
        array_to_vec(name)
    } else {
        string_vec(name).ok_or_else(|| Error::Base(format!("undefined variable: {name}")))
    }
}

/// Provide access to bash's $PIPESTATUS shell variable.
pub struct PipeStatus {
    statuses: Vec<i32>,
}

impl PipeStatus {
    /// Get the current value for $PIPESTATUS.
    pub fn get() -> Self {
        let statuses = array_to_vec("PIPESTATUS")
            .unwrap_or_default()
            .iter()
            .map(|s| s.parse::<i32>().unwrap_or(-1))
            .collect();
        Self { statuses }
    }

    /// Determine if a process failed in the related pipeline.
    pub fn failed(&self) -> bool {
        self.statuses.iter().any(|s| *s != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_and_unbind() {
        // nonexistent
        assert!(unbind("NONEXISTENT_VAR").is_ok());

        // existent
        bind("VAR", "", None, None).unwrap();
        assert_eq!(optional("VAR").unwrap(), "");
        unbind("VAR").unwrap();
        assert!(optional("VAR").is_none());

        // existent with content
        bind("VAR", "foo", None, None).unwrap();
        assert_eq!(optional("VAR").unwrap(), "foo");
        unbind("VAR").unwrap();
        assert!(optional("VAR").is_none());
    }

    #[test]
    fn test_string_vec() {
        assert!(string_vec("VAR").is_none());
        bind("VAR", "", None, None).unwrap();
        assert!(string_vec("VAR").unwrap().is_empty());
        bind("VAR", "a", None, None).unwrap();
        assert_eq!(string_vec("VAR").unwrap(), ["a"]);
        bind("VAR", "1 2 3", None, None).unwrap();
        assert_eq!(string_vec("VAR").unwrap(), ["1", "2", "3"]);
        unbind("VAR").unwrap();
        assert!(string_vec("VAR").is_none());
    }

    #[test]
    fn test_readonly_var() {
        bind("VAR", "1", None, Some(Attr::READONLY)).unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");
        let err = bind("VAR", "1", None, None).unwrap_err();
        assert_eq!(err.to_string(), "VAR: readonly variable");
        let err = unbind("VAR").unwrap_err();
        assert_eq!(err.to_string(), "VAR: cannot unset: readonly variable");
    }

    #[test]
    fn test_variable() {
        let mut var = Variable::new("VAR");
        assert_eq!(var.optional(), None);
        var.bind("", None, None).unwrap();
        assert_eq!(var.optional().unwrap(), "");
        var.bind("1", None, None).unwrap();
        assert_eq!(var.optional().unwrap(), "1");
        var.append("2").unwrap();
        assert_eq!(var.optional().unwrap(), "12");
        var.append(" 3").unwrap();
        assert_eq!(var.optional().unwrap(), "12 3");
        var.unbind().unwrap();
        assert_eq!(var.optional(), None);
    }

    #[test]
    fn test_expand() {
        let mut var1 = Variable::new("VAR1");
        let mut var2 = Variable::new("VAR2");
        var1.bind("1", None, None).unwrap();
        var2.bind("${VAR1}", None, None).unwrap();
        assert_eq!(var2.expand().unwrap(), "1");
        assert_eq!(expand("${VAR3:-3}").unwrap(), "3");
    }

    #[test]
    fn test_scoped_variable() {
        bind("VAR", "outer", None, None).unwrap();
        assert_eq!(optional("VAR").unwrap(), "outer");
        {
            let mut var = ScopedVariable::new("VAR");
            var.bind("inner", None, None).unwrap();
            assert_eq!(var.optional().unwrap(), "inner");
        }
        assert_eq!(optional("VAR").unwrap(), "outer");
    }
}
