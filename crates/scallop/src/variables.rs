use std::borrow::Borrow;
use std::ffi::{c_void, CStr, CString};
use std::fmt;

use bitflags::bitflags;
use indexmap::IndexSet;

use crate::array::Array;
use crate::error::ok_or_error;
use crate::traits::*;
use crate::{bash, Error, ExecStatus};

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

/// Return the mutable reference to a raw bash variable if it exists.
macro_rules! find_variable {
    ($name:expr) => {{
        let name = std::ffi::CString::new($name).unwrap();
        unsafe { bash::find_variable(name.as_ptr()).as_mut() }
    }};
}
pub(crate) use find_variable;

/// Unset a given variable name ignoring if it is nonexistent or readonly.
pub fn unbind<S: AsRef<str>>(name: S) -> crate::Result<ExecStatus> {
    let name = name.as_ref();
    let cstr = CString::new(name).unwrap();
    ok_or_error(|| unsafe {
        // ignore non-zero return values for nonexistent variables
        bash::unbind_variable(cstr.as_ptr());
        Ok(ExecStatus::Success)
    })
}

/// Unset a given variable name ignoring if it is nonexistent, erroring out if readonly.
pub fn unbind_check<S: AsRef<str>>(name: S) -> crate::Result<ExecStatus> {
    let name = name.as_ref();
    let cstr = CString::new(name).unwrap();
    ok_or_error(|| unsafe {
        // ignore non-zero return values for nonexistent variables
        bash::check_unbind_variable(cstr.as_ptr());
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
    ok_or_error(|| unsafe {
        if let Some(var) = bash::bind_variable(name.as_ptr(), val, flags).as_mut() {
            if let Some(attrs) = attrs {
                var.attributes |= attrs.bits() as i32;
            }
        }
        Ok(ExecStatus::Success)
    })
}

pub fn bind_global<S1, S2>(
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
    ok_or_error(|| unsafe {
        if let Some(var) = bash::bind_global_variable(name.as_ptr(), val, flags).as_mut() {
            if let Some(attrs) = attrs {
                var.attributes |= attrs.bits() as i32;
            }
        }
        Ok(ExecStatus::Success)
    })
}

pub trait ShellVariable: AsRef<str> {
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

    fn to_vec(&self) -> Option<Vec<String>> {
        var_to_vec(self.name())
    }

    fn bind<S: AsRef<str>>(
        &mut self,
        value: S,
        flags: Option<Assign>,
        attrs: Option<Attr>,
    ) -> crate::Result<ExecStatus> {
        bind(self.name(), value, flags, attrs)
    }

    fn bind_global<S: AsRef<str>>(
        &mut self,
        value: S,
        flags: Option<Assign>,
        attrs: Option<Attr>,
    ) -> crate::Result<ExecStatus> {
        bind_global(self.name(), value, flags, attrs)
    }

    fn unbind(&mut self) -> crate::Result<ExecStatus> {
        unbind(self.name())
    }

    fn append<S: AsRef<str>>(&mut self, s: S) -> crate::Result<ExecStatus> {
        self.bind(s, Some(Assign::APPEND), None)
    }

    fn is_array(&self) -> bool {
        match find_variable!(self.name()) {
            None => false,
            Some(v) => v.attributes as u32 & Attr::ARRAY.bits() != 0,
        }
    }

    fn is_readonly(&self) -> bool {
        match find_variable!(self.name()) {
            None => false,
            Some(v) => v.attributes as u32 & Attr::READONLY.bits() != 0,
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Variable {
    name: String,
}

impl Variable {
    pub fn new<S: ToString>(name: S) -> Self {
        Self { name: name.to_string() }
    }
}

impl From<&bash::ShellVar> for Variable {
    fn from(var: &bash::ShellVar) -> Self {
        let c_str = unsafe { CStr::from_ptr(var.name) };
        Self {
            name: c_str.to_string_lossy().to_string(),
        }
    }
}

impl PartialEq<str> for Variable {
    fn eq(&self, other: &str) -> bool {
        self.name() == other
    }
}

impl AsRef<str> for Variable {
    fn as_ref(&self) -> &str {
        self.name()
    }
}

impl Borrow<str> for Variable {
    fn borrow(&self) -> &str {
        self.name()
    }
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl ShellVariable for Variable {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct ScopedVariable {
    var: Variable,
    orig: Option<String>,
}

/// Variable that is reset to its original value when dropped.
impl ScopedVariable {
    /// Create a new scoped variable.
    pub fn new<S: ToString>(name: S) -> Self {
        let var = Variable::new(name);
        let orig = optional(&var.name);
        Self { var, orig }
    }

    /// Reset the variable to its original value.
    pub fn reset(&mut self) -> crate::Result<ExecStatus> {
        let result = if let Some(val) = &self.orig {
            self.var.bind(val, None, None)
        } else {
            self.var.unbind()
        };

        result.map_err(|e| Error::Base(format!("failed resetting variable: {self}: {e}")))
    }
}

impl ShellVariable for ScopedVariable {
    fn name(&self) -> &str {
        &self.var.name
    }
}

impl AsRef<str> for ScopedVariable {
    fn as_ref(&self) -> &str {
        self.name()
    }
}

impl fmt::Display for ScopedVariable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl Drop for ScopedVariable {
    fn drop(&mut self) {
        if self.optional() != self.orig {
            self.reset().unwrap();
        }
    }
}

/// Get the raw string value of a given variable name, returning None when nonexistent.
pub fn optional<S: AsRef<str>>(name: S) -> Option<String> {
    let name = CString::new(name.as_ref()).unwrap();
    unsafe {
        bash::get_string_value(name.as_ptr())
            .as_ref()
            .map(|s| CStr::from_ptr(s).to_str().unwrap().to_string())
    }
}

/// Get the raw string value of a given variable name, returning an Error when nonexistent.
pub fn required<S: AsRef<str>>(name: S) -> crate::Result<String> {
    let name = name.as_ref();
    optional(name).ok_or_else(|| Error::Base(format!("undefined variable: {name}")))
}

/// Get the expanded value of a given string.
pub fn expand<S: AsRef<str>>(val: S) -> Option<String> {
    let val = CString::new(val.as_ref()).unwrap();
    unsafe {
        bash::expand_string_to_string(val.as_ptr() as *mut _, 0)
            .as_ref()
            .map(|s| CStr::from_ptr(s).to_str().unwrap().to_string())
    }
}

/// Expand an iterable of strings applying various substitutions.
pub fn expand_iter<I, S>(vals: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let words: Words = vals.into_iter().collect();
    let ptr: *mut bash::WordList = (&words).into();
    unsafe {
        bash::expand_words_no_vars(ptr)
            .into_words()
            .try_into()
            .unwrap()
    }
}

/// Perform file globbing on a string.
pub fn glob_files<S: AsRef<str>>(val: S) -> Vec<String> {
    let mut files = vec![];
    let val = CString::new(val.as_ref()).unwrap();
    unsafe {
        let paths = bash::shell_glob_filename(val.as_ptr() as *mut _, 0);
        if !paths.is_null() {
            let mut i = 0;
            while let Some(s) = (*paths.offset(i)).as_ref() {
                files.push(CStr::from_ptr(s).to_string_lossy().into());
                i += 1;
            }
        }
    }
    files
}

/// Get the string value of a given variable name splitting it into Vec<String> based on IFS.
pub fn string_vec<S: AsRef<str>>(name: S) -> Option<Vec<String>> {
    let name = name.as_ref();
    let var_name = CString::new(name).unwrap();
    unsafe {
        bash::get_string_value(var_name.as_ptr()).as_mut().map(|s| {
            bash::list_string(s, bash::IFS, 1)
                .into_words()
                .try_into()
                .unwrap()
        })
    }
}

/// Get the value of a given variable as Vec<String>.
pub fn var_to_vec<S: AsRef<str>>(name: S) -> Option<Vec<String>> {
    let name = name.as_ref();
    match Array::from(name) {
        Ok(array) => Some(array.into_iter().collect()),
        Err(_) => string_vec(name),
    }
}

/// Convert an array of bash shell variable pointers into Vec<Variable>.
macro_rules! shell_variables {
    ($variables:expr) => {{
        let mut vars = IndexSet::new();
        unsafe {
            let shell_vars = $variables;
            if !shell_vars.is_null() {
                let mut i = 0;
                while let Some(var) = (*shell_vars.offset(i)).as_ref() {
                    vars.insert(var.into());
                    i += 1;
                }
                bash::xfree(shell_vars as *mut c_void);
            }
        }
        vars
    }};
}

/// Return the ordered set of all shell variables.
pub fn all() -> IndexSet<Variable> {
    shell_variables!(bash::all_shell_variables())
}

/// Return the ordered set of all visible shell variables.
pub fn visible() -> IndexSet<Variable> {
    shell_variables!(bash::all_visible_variables())
}

/// Return the ordered set of all exported shell variables.
pub fn exported() -> IndexSet<Variable> {
    shell_variables!(bash::all_exported_variables())
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
        bind("VAR", "1", None, None).unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");
        unbind("VAR").unwrap();
        assert!(optional("VAR").is_none());

        // unbind readonly
        bind("VAR", "2", None, Some(Attr::READONLY)).unwrap();
        assert_eq!(optional("VAR").unwrap(), "2");
        assert!(unbind_check("VAR").is_err());
        assert!(unbind("VAR").is_ok());
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

        // binding fails
        let err = bind("VAR", "1", None, None).unwrap_err();
        assert_eq!(err.to_string(), "VAR: readonly variable");

        // checked unbind fails
        let err = unbind_check("VAR").unwrap_err();
        assert_eq!(err.to_string(), "VAR: cannot unset: readonly variable");

        // forced unbind succeeds
        assert!(unbind("VAR").is_ok());
    }

    #[test]
    fn test_variable() {
        let mut var = Variable::new("VAR");
        assert_eq!(var.as_ref(), "VAR");
        assert_eq!(var.to_string(), "VAR");
        assert_eq!(var.optional(), None);
        assert!(var.required().is_err());
        assert!(!var.is_readonly());
        assert!(!var.is_array());

        var.bind("", None, None).unwrap();
        assert_eq!(var.optional().unwrap(), "");
        assert_eq!(var.required().unwrap(), "");

        var.bind("1", None, None).unwrap();
        assert_eq!(var.optional().unwrap(), "1");
        assert_eq!(var.required().unwrap(), "1");

        var.append("2").unwrap();
        assert_eq!(var.optional().unwrap(), "12");
        assert_eq!(var.required().unwrap(), "12");

        var.append(" 3").unwrap();
        assert_eq!(var.optional().unwrap(), "12 3");
        assert_eq!(var.required().unwrap(), "12 3");

        var.bind("", None, Some(Attr::READONLY)).unwrap();
        assert_eq!(var.optional().unwrap(), "");
        assert_eq!(var.required().unwrap(), "");
        assert!(var.is_readonly());

        var.unbind().unwrap();
        assert_eq!(var.optional(), None);
        assert!(var.required().is_err());

        var.bind_global("4", None, Some(Attr::READONLY)).unwrap();
        assert_eq!(var.optional().unwrap(), "4");
        assert_eq!(var.required().unwrap(), "4");
        assert!(var.bind_global("5", None, None).is_err());
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
    fn scoped_variable() {
        bind("VAR", "outer", None, None).unwrap();
        assert_eq!(optional("VAR").unwrap(), "outer");
        {
            let mut var = ScopedVariable::new("VAR");
            assert_eq!(var.as_ref(), "VAR");
            var.bind("inner", None, None).unwrap();
            assert_eq!(var.optional().unwrap(), "inner");
        }
        assert_eq!(optional("VAR").unwrap(), "outer");
    }

    #[test]
    fn test_sets() {
        assert!(!all().iter().any(|s| s == "SCALLOP_VAR_TEST"));
        assert!(!visible().iter().any(|s| s == "SCALLOP_VAR_TEST"));
        assert!(!exported().iter().any(|s| s == "SCALLOP_VAR_TEST"));
        bind("SCALLOP_VAR_TEST", "1", None, None).unwrap();
        assert!(all().iter().any(|s| s == "SCALLOP_VAR_TEST"));
        assert!(visible().iter().any(|s| s == "SCALLOP_VAR_TEST"));
        assert!(!exported().iter().any(|s| s == "SCALLOP_VAR_TEST"));
        bind("SCALLOP_VAR_TEST", "1", None, Some(Attr::EXPORTED)).unwrap();
        assert!(exported().iter().any(|s| s == "SCALLOP_VAR_TEST"));
    }
}
