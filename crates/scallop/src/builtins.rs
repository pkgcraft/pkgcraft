use std::borrow::Borrow;
use std::ffi::{CStr, CString, c_int};
use std::hash::{Hash, Hasher};
use std::{cmp, fmt, mem, process, ptr};

use bitflags::bitflags;
use indexmap::IndexSet;

use crate::error::{Error, ok_or_error};
use crate::macros::*;
use crate::traits::{IntoWords, Words};
use crate::{ExecStatus, bash, shell};

mod _bash;
mod _profile;
#[cfg(test)]
mod _scallop;
mod _sleep;

// export native bash builtins
pub use _bash::*;

// export builtins for external registry
pub use _profile::BUILTIN as profile;
pub use _sleep::BUILTIN as sleep;

pub type BuiltinFn = fn(&[&str]) -> crate::Result<ExecStatus>;
pub type BuiltinFnPtr = unsafe extern "C" fn(list: *mut bash::WordList) -> c_int;

bitflags! {
    /// Flag values describing builtin attributes.
    #[derive(Clone, Copy)]
    pub struct Attr: u32 {
        const NONE = 0;
        const ENABLED = bash::BUILTIN_ENABLED;
        const DELETED = bash::BUILTIN_DELETED;
        const STATIC = bash::STATIC_BUILTIN;
        const SPECIAL = bash::SPECIAL_BUILTIN;
        const ASSIGNMENT = bash::ASSIGNMENT_BUILTIN;
        const POSIX = bash::POSIX_BUILTIN;
        const LOCALVAR = bash::LOCALVAR_BUILTIN;
        const ARRAYREF = bash::ARRAYREF_BUILTIN;
    }
}

pub mod set {
    use super::*;

    pub fn enable<'a, I, S>(opts: I) -> crate::Result<ExecStatus>
    where
        I: IntoIterator<Item = &'a S>,
        S: AsRef<str> + 'a + ?Sized,
    {
        set(["-o"]
            .into_iter()
            .chain(opts.into_iter().map(|s| s.as_ref())))
    }

    pub fn disable<'a, I, S>(opts: I) -> crate::Result<ExecStatus>
    where
        I: IntoIterator<Item = &'a S>,
        S: AsRef<str> + 'a + ?Sized,
    {
        set(["+o"]
            .into_iter()
            .chain(opts.into_iter().map(|s| s.as_ref())))
    }
}

pub mod shopt {
    use super::*;

    pub fn enable<'a, I, S>(opts: I) -> crate::Result<ExecStatus>
    where
        I: IntoIterator<Item = &'a S>,
        S: AsRef<str> + 'a + ?Sized,
    {
        shopt(
            ["-s"]
                .into_iter()
                .chain(opts.into_iter().map(|s| s.as_ref())),
        )
    }

    pub fn disable<'a, I, S>(opts: I) -> crate::Result<ExecStatus>
    where
        I: IntoIterator<Item = &'a S>,
        S: AsRef<str> + 'a + ?Sized,
    {
        shopt(
            ["-u"]
                .into_iter()
                .chain(opts.into_iter().map(|s| s.as_ref())),
        )
    }
}

#[derive(Clone, Copy)]
pub struct Builtin {
    pub name: &'static str,
    pub func: BuiltinFn,
    pub cfunc: BuiltinFnPtr,
    pub flags: Attr,
    pub help: &'static str,
    pub usage: &'static str,
}

impl Builtin {
    // TODO: Implement callable trait support if it's ever stabilized
    // https://github.com/rust-lang/rust/issues/29625
    /// Call the builtin with the given arguments.
    pub fn call(&self, args: &[&str]) -> crate::Result<ExecStatus> {
        (self.func)(args)
    }
}

impl fmt::Debug for Builtin {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Builtin").field("name", &self.name).finish()
    }
}

impl fmt::Display for Builtin {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl PartialEq for Builtin {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Builtin {}

impl Hash for Builtin {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Ord for Builtin {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.name.cmp(other.name)
    }
}

impl PartialOrd for Builtin {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl AsRef<str> for Builtin {
    fn as_ref(&self) -> &str {
        self.name
    }
}

impl Borrow<str> for Builtin {
    fn borrow(&self) -> &str {
        self.name
    }
}

impl From<Builtin> for bash::Builtin {
    fn from(builtin: Builtin) -> bash::Builtin {
        let name_str = CString::new(builtin.name).unwrap();
        let name = name_str.as_ptr() as *mut _;
        mem::forget(name_str);

        let short_doc_str = CString::new(builtin.usage).unwrap();
        let short_doc = short_doc_str.as_ptr();
        mem::forget(short_doc_str);

        let long_docs = iter_to_array!(builtin.help.lines(), str_to_raw);
        let long_doc = long_docs.as_ptr();
        mem::forget(long_docs);

        // register as static builtin
        let flags = builtin.flags | Attr::STATIC;

        bash::Builtin {
            name,
            function: Some(builtin.cfunc),
            flags: flags.bits() as i32,
            long_doc,
            short_doc,
            handle: ptr::null_mut(),
        }
    }
}

/// Wrapper for a registered bash builtin.
pub(crate) struct BashBuiltin(&'static mut bash::Builtin);

impl BashBuiltin {
    /// Call the builtin with the given arguments.
    pub(crate) fn call<I>(&self, args: I) -> crate::Result<ExecStatus>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        // convert args to bash word list
        let args: Words = args.into_iter().collect();

        // stub builtins for reserved keywords with null functions can't get here
        let function = self.0.function.unwrap();

        // Update global variables used to track execution state as similarly done in
        // the `builtin` builtin before running the target builtin.
        unsafe {
            bash::CURRENT_COMMAND_NAME = self.0.name;
            bash::CURRENT_BUILTIN_FUNC = Some(function);
        }

        ok_or_error(|| {
            let ret = unsafe { function(args.as_ptr()) };
            if ret == 0 {
                Ok(ExecStatus::Success)
            } else {
                Err(Error::Base(format!("failed running {self} builtin: exit status {ret}")))
            }
        })
    }

    /// Return the bash builtin for a given name.
    pub(crate) fn find<S: AsRef<str>>(name: S) -> crate::Result<Self> {
        let name = name.as_ref();
        let builtin_name = CString::new(name).unwrap();
        let builtin_ptr = builtin_name.as_ptr() as *mut _;
        let builtin = unsafe {
            // search for registered builtin
            let builtin = bash::builtin_address_internal(builtin_ptr, 1);
            // Update global variable used to track execution state as similarly done in
            // bash builtin search functionality such as find_shell_builtin().
            bash::CURRENT_BUILTIN = builtin;
            builtin.as_mut().map(Self)
        };

        builtin.ok_or_else(|| Error::Base(format!("unknown builtin: {name}")))
    }

    /// Return true if the builtin is enabled, otherwise false.
    pub(crate) fn is_enabled(&self) -> bool {
        self.0.flags & Attr::ENABLED.bits() as i32 == 1
    }

    /// Return true if the builtin is a reserved keyword, e.g. `while`.
    fn is_keyword(&self) -> bool {
        self.0.function.is_none()
    }

    /// Enable or disable the builtin.
    pub(crate) fn enable(&mut self, status: bool) {
        if status {
            self.0.flags |= Attr::ENABLED.bits() as i32;
        } else {
            self.0.flags &= !Attr::ENABLED.bits() as i32;
        }
    }

    /// Return the builtin's name.
    fn name(&self) -> &str {
        let name = unsafe { CStr::from_ptr(self.0.name) };
        name.to_str().unwrap()
    }
}

impl fmt::Debug for BashBuiltin {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BashBuiltin")
            .field("name", &self.name())
            .finish()
    }
}

impl fmt::Display for BashBuiltin {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Enable or disable function overriding for an iterable of builtins.
pub fn override_funcs<I>(builtins: I, enable: bool) -> crate::Result<()>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    for name in builtins {
        let builtin = BashBuiltin::find(name)?;
        if enable {
            builtin.0.flags |= Attr::SPECIAL.bits() as i32;
        } else {
            builtin.0.flags &= !Attr::SPECIAL.bits() as i32;
        }
    }

    Ok(())
}

/// Toggle an iterable of builtins, returning those were.
fn toggle_status<I, S>(builtins: I, enable: bool) -> crate::Result<Vec<S>>
where
    S: AsRef<str>,
    I: IntoIterator<Item = S>,
{
    let mut toggled = vec![];

    for name in builtins {
        let mut builtin = BashBuiltin::find(&name)?;
        if builtin.is_enabled() != enable {
            builtin.enable(enable);
            toggled.push(name);
        }
    }

    Ok(toggled)
}

/// Disable a given list of builtins by name.
pub fn disable<I, S>(builtins: I) -> crate::Result<Vec<S>>
where
    S: AsRef<str>,
    I: IntoIterator<Item = S>,
{
    toggle_status(builtins, false)
}

/// Enable a given list of builtins by name.
pub fn enable<I, S>(builtins: I) -> crate::Result<Vec<S>>
where
    S: AsRef<str>,
    I: IntoIterator<Item = S>,
{
    toggle_status(builtins, true)
}

/// Get the sets of enabled and disabled shell builtins.
pub fn shell_builtins() -> (IndexSet<String>, IndexSet<String>) {
    let mut enabled = IndexSet::new();
    let mut disabled = IndexSet::new();

    let end = unsafe { (bash::NUM_SHELL_BUILTINS - 1) as isize };
    for i in 0..end {
        let builtin = unsafe { bash::SHELL_BUILTINS.offset(i).as_mut().unwrap() };
        let builtin = BashBuiltin(builtin);
        if !builtin.is_keyword() {
            if builtin.is_enabled() {
                enabled.insert(builtin.to_string());
            } else {
                disabled.insert(builtin.to_string());
            }
        }
    }

    (enabled, disabled)
}

/// Register builtins into the internal list for use.
pub fn register<I>(builtins: I)
where
    I: IntoIterator,
    I::Item: Into<Builtin>,
{
    // convert builtins into pointers
    let mut builtin_ptrs: Vec<_> = builtins
        .into_iter()
        .map(Into::into)
        .map(|b| Box::into_raw(Box::new(b.into())))
        .collect();

    unsafe {
        // add builtins to bash's internal list
        bash::register_builtins(builtin_ptrs.as_mut_ptr(), builtin_ptrs.len());

        // reclaim pointers for proper deallocation
        for b in builtin_ptrs {
            mem::drop(Box::from_raw(b));
        }
    }
}

/// Enable/disable builtins, automatically reverting their status when leaving scope.
#[derive(Debug, Default)]
pub struct ScopedBuiltins<S: AsRef<str>> {
    names: Vec<S>,
    enabled: bool,
}

impl<S: AsRef<str>> ScopedBuiltins<S> {
    pub fn disable<I>(values: I) -> crate::Result<Self>
    where
        I: IntoIterator<Item = S>,
    {
        Ok(Self {
            names: disable(values)?,
            enabled: false,
        })
    }

    pub fn enable<I>(values: I) -> crate::Result<Self>
    where
        I: IntoIterator<Item = S>,
    {
        Ok(Self {
            names: enable(values)?,
            enabled: true,
        })
    }
}

impl<S: AsRef<str>> Drop for ScopedBuiltins<S> {
    fn drop(&mut self) {
        if self.enabled {
            disable(&self.names).unwrap_or_else(|_| panic!("failed disabling builtins"));
        } else {
            enable(&self.names).unwrap_or_else(|_| panic!("failed enabling builtins"));
        }
    }
}

/// Toggle shell options, automatically reverting their status when leaving scope.
#[derive(Debug, Default)]
pub struct ScopedOptions {
    shopt_enabled: IndexSet<String>,
    shopt_disabled: IndexSet<String>,
    set_enabled: IndexSet<String>,
    set_disabled: IndexSet<String>,
}

impl ScopedOptions {
    /// Enable shell options.
    pub fn enable<I, S>(&mut self, options: I) -> crate::Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let enabled_shopt = bash::shopt_opts();
        let enabled_set = bash::set_opts();

        for opt in options {
            let opt = opt.as_ref();
            if bash::SET_OPTS.contains(opt) {
                if !enabled_set.contains(opt) && self.set_enabled.insert(opt.into()) {
                    set::enable([opt])?;
                }
            } else if bash::SHOPT_OPTS.contains(opt) {
                if !enabled_shopt.contains(opt) && self.shopt_enabled.insert(opt.into()) {
                    shopt::enable([opt])?;
                }
            } else {
                return Err(Error::Base(format!("unknown option: {opt}")));
            }
        }

        Ok(())
    }

    /// Disable shell options.
    pub fn disable<I, S>(&mut self, options: I) -> crate::Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let enabled_shopt = bash::shopt_opts();
        let enabled_set = bash::set_opts();

        for opt in options {
            let opt = opt.as_ref();
            if bash::SET_OPTS.contains(opt) {
                if enabled_set.contains(opt) && self.set_disabled.insert(opt.into()) {
                    set::disable([opt])?;
                }
            } else if bash::SHOPT_OPTS.contains(opt) {
                if enabled_shopt.contains(opt) && self.shopt_disabled.insert(opt.into()) {
                    shopt::disable([opt])?;
                }
            } else {
                return Err(Error::Base(format!("unknown option: {opt}")));
            }
        }

        Ok(())
    }
}

impl Drop for ScopedOptions {
    fn drop(&mut self) {
        if !self.shopt_enabled.is_empty() {
            shopt::disable(&self.shopt_enabled).expect("failed unsetting shopt options");
        }

        if !self.shopt_disabled.is_empty() {
            shopt::enable(&self.shopt_disabled).expect("failed setting shopt options");
        }

        if !self.set_enabled.is_empty() {
            set::disable(&self.set_enabled).expect("failed unsetting set options");
        }

        if !self.set_disabled.is_empty() {
            set::enable(&self.set_disabled).expect("failed setting set options");
        }
    }
}

/// Handle builtin errors.
pub fn handle_error<S: AsRef<str>>(cmd: S, err: Error) -> ExecStatus {
    let mut msg = String::new();

    // add error line prolog for relevant line numbers
    let lineno = shell::executing_line_number();
    if lineno > 0 {
        msg.push_str(&format!("line {lineno}: "));
    }

    // append command prefix for relevant builtin errors lacking it
    let err_msg = err.to_string();
    let cmd = cmd.as_ref();
    let cmd_prefix = format!("{cmd}: error: ");
    if cmd != "command_not_found_handle" && !err_msg.starts_with(&cmd_prefix) {
        msg.push_str(&cmd_prefix);
    }

    // append builtin error message
    msg.push_str(&err_msg);

    let bail = matches!(err, Error::Bail(_));
    // push error message into shared memory so subshell errors can be captured
    shell::set_shm_error(&msg, bail);

    // exit subshell with status causing the main process to longjmp to the entry point
    if bail && !shell::in_main() {
        process::exit(bash::EX_LONGJMP as i32);
    }

    ExecStatus::from(err)
}

/// Run a builtin as called from bash.
fn run(builtin: &Builtin, args: *mut bash::WordList) -> ExecStatus {
    // convert raw command args into &str
    let args = args.to_words();
    let args: Result<Vec<_>, _> = args.into_iter().collect();

    // run command if args are valid utf8
    let result = match args {
        Ok(args) => builtin.call(&args),
        Err(e) => Err(Error::Base(format!("invalid args: {e}"))),
    };

    // handle builtin errors extracting the return status
    result.unwrap_or_else(|e| handle_error(builtin, e))
}

/// Create C compatible builtin function wrapper converting between rust and C types.
#[macro_export]
macro_rules! make_builtin {
    ($name:expr, $func_name:ident, $func:expr, $long_doc:expr, $usage:expr) => {
        use std::ffi::c_int;

        use $crate::builtins::Builtin;

        #[unsafe(no_mangle)]
        extern "C" fn $func_name(args: *mut $crate::bash::WordList) -> c_int {
            i32::from($crate::builtins::run(&BUILTIN, args))
        }

        // ignore unreachable test builtins
        #[allow(unreachable_pub)]
        pub static BUILTIN: Builtin = Builtin {
            name: $name,
            func: $func,
            cfunc: $func_name,
            flags: $crate::builtins::Attr::NONE,
            help: $long_doc,
            usage: $usage,
        };
    };
}
pub use make_builtin;

#[cfg(test)]
mod tests {
    use crate::test::assert_err_re;
    use crate::{source, variables};

    use super::*;

    #[test]
    fn traits() {
        // PartialEq and PartialOrd
        assert!(profile == profile);
        assert!(profile >= profile);

        // Display, Debug, and AsRef<str>
        assert_eq!(profile.to_string(), "profile");
        assert!(format!("{profile:?}").contains("profile"));
        assert_eq!(profile.as_ref(), "profile");

        // Hash and Borrow<str>
        let builtins = IndexSet::from([profile]);
        assert_eq!(builtins.get("profile").unwrap(), &profile);
    }

    #[test]
    fn toggle_builtins() {
        // select a builtin to toggle
        let (enabled, disabled) = shell_builtins();
        assert!(!enabled.is_empty());
        let builtin = enabled.iter().next().unwrap();
        assert!(!disabled.contains(builtin));

        // disable the builtin
        disable([builtin]).unwrap();
        let (enabled, disabled) = shell_builtins();
        assert!(!enabled.contains(builtin));
        assert!(disabled.contains(builtin));

        // enable the builtin
        enable([builtin]).unwrap();
        let (enabled, disabled) = shell_builtins();
        assert!(enabled.contains(builtin));
        assert!(!disabled.contains(builtin));

        // unknown builtin
        assert!(enable(["nonexistent"]).is_err());
        assert!(disable(["nonexistent"]).is_err());
    }

    #[test]
    fn toggle_overrides() {
        variables::bind_global("VAR", "1", None, None).unwrap();

        // functions override builtins by default
        source::string("declare() { (( VAR += 1 )); }").unwrap();
        source::string("declare").unwrap();
        assert_eq!(variables::optional("VAR").unwrap(), "2");

        // builtins marked as special override functions
        override_funcs(["declare"], true).unwrap();
        source::string("declare").unwrap();
        assert_eq!(variables::optional("VAR").unwrap(), "2");

        // revert to functions overriding builtins
        override_funcs(["declare"], false).unwrap();
        source::string("declare").unwrap();
        assert_eq!(variables::optional("VAR").unwrap(), "3");

        // unknown builtin
        assert!(override_funcs(["nonexistent"], true).is_err());
    }

    #[test]
    fn scoped_builtins() {
        assert!(source::string("declare").is_ok());
        let _builtins = ScopedBuiltins::disable(["declare"]).unwrap();
        assert!(source::string("declare").is_err());
        let _builtins = ScopedBuiltins::enable(["declare"]).unwrap();
        assert!(source::string("declare").is_ok());
    }

    #[test]
    fn scoped_options() {
        // invalid options
        let mut opts = ScopedOptions::default();
        assert!(opts.enable(["unknown"]).is_err());
        assert!(opts.disable(["unknown"]).is_err());

        // shopt options
        let (enable, disable) = ("autocd", "sourcepath");
        shopt::disable([enable]).unwrap();
        shopt::enable([disable]).unwrap();

        assert!(!bash::shopt_opts().contains(enable));
        assert!(bash::shopt_opts().contains(disable));
        {
            let mut opts = ScopedOptions::default();
            // perform twice to complete branch coverage
            opts.enable([enable]).unwrap();
            opts.enable([enable]).unwrap();
            opts.disable([disable]).unwrap();
            opts.disable([disable]).unwrap();
            assert!(bash::shopt_opts().contains(enable));
            assert!(!bash::shopt_opts().contains(disable));
        }
        assert!(!bash::shopt_opts().contains(enable));
        assert!(bash::shopt_opts().contains(disable));

        // set options
        let (enable, disable) = ("noglob", "verbose");
        set::disable([enable]).unwrap();
        set::enable([disable]).unwrap();

        assert!(!bash::set_opts().contains(enable));
        assert!(bash::set_opts().contains(disable));
        {
            let mut opts = ScopedOptions::default();
            // perform twice to complete branch coverage
            opts.enable([enable]).unwrap();
            opts.enable([enable]).unwrap();
            opts.disable([disable]).unwrap();
            opts.disable([disable]).unwrap();
            assert!(bash::set_opts().contains(enable));
            assert!(!bash::set_opts().contains(disable));
        }
        assert!(!bash::set_opts().contains(enable));
        assert!(bash::set_opts().contains(disable));
    }

    #[test]
    fn bash_builtin() {
        // nonexistent
        let r = BashBuiltin::find("nonexistent");
        assert_err_re!(r, "unknown builtin: nonexistent");

        // reserved keyword
        let r = BashBuiltin::find("while");
        assert_err_re!(r, "unknown builtin: while");

        let (enabled, _disabled) = shell_builtins();
        // enabled
        let name = enabled.iter().next().unwrap();
        let mut builtin = BashBuiltin::find(name).unwrap();
        assert_eq!(name, builtin.name());
        assert!(format!("{builtin:?}").contains(name));
        assert!(builtin.is_enabled());
        // disabled
        builtin.enable(false);
        assert!(!builtin.is_enabled());
        let disabled = BashBuiltin::find(name).unwrap();
        assert!(!disabled.is_enabled());
    }
}
