use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{cmp, fmt};

use indexmap::{IndexMap, IndexSet};
use once_cell::sync::Lazy;
use scallop::{Error, ExecStatus};

use crate::eapi::{self, Eapi, EAPIS};

use super::get_build_mut;
use super::phase::PhaseKind;
use super::scope::{Scope, Scopes};

mod _default_phase_func;
mod _new;
mod _phases;
mod _query_cmd;
mod _use_conf;
mod adddeny;
mod addpredict;
mod addread;
mod addwrite;
mod assert;
mod best_version;
mod command_not_found_handle;
mod debug_print;
mod debug_print_function;
mod debug_print_section;
mod default;
mod default_pkg_nofetch;
mod default_src_compile;
mod default_src_configure;
mod default_src_install;
mod default_src_prepare;
mod default_src_test;
mod default_src_unpack;
mod die;
mod diropts;
mod dobin;
mod docinto;
mod docompress;
mod doconfd;
mod dodir;
mod dodoc;
mod doenvd;
mod doexe;
mod doheader;
mod dohtml;
mod doinfo;
mod doinitd;
mod doins;
mod dolib;
mod dolib_a;
mod dolib_so;
mod doman;
mod domo;
mod dosbin;
mod dostrip;
mod dosym;
pub(super) mod eapply;
pub(super) mod eapply_user;
mod ebegin;
pub(super) mod econf;
mod eend;
mod eerror;
mod einfo;
mod einfon;
mod einstall;
pub(super) mod einstalldocs;
mod elog;
pub(super) mod emake;
mod eqawarn;
mod ewarn;
mod exeinto;
mod exeopts;
mod export_functions;
mod fowners;
mod fperms;
mod get_libdir;
mod has;
mod has_version;
mod hasq;
mod hasv;
mod in_iuse;
mod inherit;
mod insinto;
mod insopts;
mod into;
mod keepdir;
mod libopts;
mod newbin;
mod newconfd;
mod newdoc;
mod newenvd;
mod newexe;
mod newheader;
mod newinitd;
mod newins;
mod newlib_a;
mod newlib_so;
mod newman;
mod newsbin;
mod nonfatal;
pub(super) mod unpack;
mod use_;
mod use_enable;
mod use_with;
mod useq;
mod usev;
mod usex;
mod ver_cut;
mod ver_rs;
mod ver_test;

#[derive(Debug)]
pub struct Builtin {
    builtin: scallop::builtins::Builtin,
    deprecated: Option<&'static Eapi>,
    scope: IndexMap<&'static Eapi, IndexSet<Scope>>,
}

impl From<&&Builtin> for scallop::builtins::Builtin {
    fn from(b: &&Builtin) -> Self {
        b.builtin
    }
}

impl Builtin {
    fn new<'a, I, J, S>(
        builtin: scallop::builtins::Builtin,
        valid: I,
        deprecated: Option<&'static Eapi>,
    ) -> Self
    where
        I: IntoIterator<Item = (&'a str, J)>,
        J: IntoIterator<Item = S>,
        S: Into<Scopes>,
    {
        let mut scope = IndexMap::new();
        for (range, scopes) in valid {
            let mut scopes: IndexSet<_> = scopes.into_iter().flat_map(Into::into).collect();
            scopes.sort();
            let eapis: Vec<_> = eapi::range(range)
                .unwrap_or_else(|e| panic!("{builtin}: failed parsing EAPI range: {range}: {e}"))
                .collect();

            if eapis.is_empty() {
                panic!("{builtin}: no supported EAPIs in range: {range}");
            }

            for eapi in eapis {
                if scope.insert(eapi, scopes.clone()).is_some() {
                    panic!("{builtin}: EAPI {eapi} has clashing scopes");
                }
            }
        }

        Builtin { builtin, deprecated, scope }
    }

    /// Determine if the builtin is deprecated for a given EAPI.
    pub fn is_deprecated(&self, eapi: &Eapi) -> bool {
        self.deprecated.as_ref().is_some_and(|e| eapi >= e)
    }

    /// Determine if the builtin is enabled for a given EAPI.
    fn is_enabled(&self, eapi: &Eapi) -> bool {
        self.scope.contains_key(eapi)
    }

    /// Determine if the builtin is a phase stub.
    fn is_phase(&self) -> bool {
        PhaseKind::from_str(self.builtin.name).is_ok()
    }

    /// Run a builtin if it's enabled for the current build state.
    fn run(&self, args: &[&str]) -> scallop::Result<ExecStatus> {
        let build = get_build_mut();
        let eapi = build.eapi();
        let scope = &build.scope;

        match self.scope.get(eapi) {
            Some(s) if s.contains(scope) => self.builtin.run(args),
            Some(_) => Err(Error::Base(format!("disabled in {scope} scope"))),
            None => Err(Error::Base(format!("disabled in EAPI {eapi}"))),
        }
    }
}

impl AsRef<str> for Builtin {
    fn as_ref(&self) -> &str {
        self.builtin.name
    }
}

impl Eq for Builtin {}

impl PartialEq for Builtin {
    fn eq(&self, other: &Self) -> bool {
        self.builtin.name == other.builtin.name
    }
}

impl Hash for Builtin {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.builtin.name.hash(state);
    }
}

impl Ord for Builtin {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.builtin.name.cmp(other.builtin.name)
    }
}

impl PartialOrd for Builtin {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Borrow<str> for &Builtin {
    fn borrow(&self) -> &str {
        self.builtin.name
    }
}

impl fmt::Display for Builtin {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

/// Ordered set of all known builtins.
pub(crate) static BUILTINS: Lazy<IndexSet<&Builtin>> = Lazy::new(|| {
    [
        &*adddeny::BUILTIN,
        &*addpredict::BUILTIN,
        &*addread::BUILTIN,
        &*addwrite::BUILTIN,
        &*assert::BUILTIN,
        &*best_version::BUILTIN,
        &*command_not_found_handle::BUILTIN,
        &*debug_print::BUILTIN,
        &*debug_print_function::BUILTIN,
        &*debug_print_section::BUILTIN,
        &*default::BUILTIN,
        &*default_pkg_nofetch::BUILTIN,
        &*default_src_compile::BUILTIN,
        &*default_src_configure::BUILTIN,
        &*default_src_install::BUILTIN,
        &*default_src_prepare::BUILTIN,
        &*default_src_test::BUILTIN,
        &*default_src_unpack::BUILTIN,
        &*die::BUILTIN,
        &*diropts::BUILTIN,
        &*dobin::BUILTIN,
        &*docinto::BUILTIN,
        &*docompress::BUILTIN,
        &*doconfd::BUILTIN,
        &*dodir::BUILTIN,
        &*dodoc::BUILTIN,
        &*doenvd::BUILTIN,
        &*doexe::BUILTIN,
        &*doheader::BUILTIN,
        &*dohtml::BUILTIN,
        &*doinfo::BUILTIN,
        &*doinitd::BUILTIN,
        &*doins::BUILTIN,
        &*dolib::BUILTIN,
        &*dolib_a::BUILTIN,
        &*dolib_so::BUILTIN,
        &*doman::BUILTIN,
        &*domo::BUILTIN,
        &*dosbin::BUILTIN,
        &*dostrip::BUILTIN,
        &*dosym::BUILTIN,
        &*eapply::BUILTIN,
        &*eapply_user::BUILTIN,
        &*ebegin::BUILTIN,
        &*econf::BUILTIN,
        &*eend::BUILTIN,
        &*eerror::BUILTIN,
        &*einfo::BUILTIN,
        &*einfon::BUILTIN,
        &*einstall::BUILTIN,
        &*einstalldocs::BUILTIN,
        &*elog::BUILTIN,
        &*emake::BUILTIN,
        &*eqawarn::BUILTIN,
        &*ewarn::BUILTIN,
        &*exeinto::BUILTIN,
        &*exeopts::BUILTIN,
        &*export_functions::BUILTIN,
        &*fowners::BUILTIN,
        &*fperms::BUILTIN,
        &*get_libdir::BUILTIN,
        &*has::BUILTIN,
        &*has_version::BUILTIN,
        &*hasq::BUILTIN,
        &*hasv::BUILTIN,
        &*in_iuse::BUILTIN,
        &*inherit::BUILTIN,
        &*insinto::BUILTIN,
        &*insopts::BUILTIN,
        &*into::BUILTIN,
        &*keepdir::BUILTIN,
        &*libopts::BUILTIN,
        &*newbin::BUILTIN,
        &*newconfd::BUILTIN,
        &*newdoc::BUILTIN,
        &*newenvd::BUILTIN,
        &*newexe::BUILTIN,
        &*newheader::BUILTIN,
        &*newinitd::BUILTIN,
        &*newins::BUILTIN,
        &*newlib_a::BUILTIN,
        &*newlib_so::BUILTIN,
        &*newman::BUILTIN,
        &*newsbin::BUILTIN,
        &*nonfatal::BUILTIN,
        &*unpack::BUILTIN,
        &*use_::BUILTIN,
        &*use_enable::BUILTIN,
        &*use_with::BUILTIN,
        &*useq::BUILTIN,
        &*usev::BUILTIN,
        &*usex::BUILTIN,
        &*ver_cut::BUILTIN,
        &*ver_rs::BUILTIN,
        &*ver_test::BUILTIN,
        &*_phases::PKG_CONFIG_BUILTIN,
        &*_phases::PKG_INFO_BUILTIN,
        &*_phases::PKG_NOFETCH_BUILTIN,
        &*_phases::PKG_POSTINST_BUILTIN,
        &*_phases::PKG_POSTRM_BUILTIN,
        &*_phases::PKG_PREINST_BUILTIN,
        &*_phases::PKG_PRERM_BUILTIN,
        &*_phases::PKG_PRETEND_BUILTIN,
        &*_phases::PKG_SETUP_BUILTIN,
        &*_phases::SRC_COMPILE_BUILTIN,
        &*_phases::SRC_CONFIGURE_BUILTIN,
        &*_phases::SRC_INSTALL_BUILTIN,
        &*_phases::SRC_PREPARE_BUILTIN,
        &*_phases::SRC_TEST_BUILTIN,
        &*_phases::SRC_UNPACK_BUILTIN,
    ]
    .into_iter()
    .collect()
});

/// Ordered mapping of EAPIs to builtin names that they enable.
pub(crate) static EAPI_BUILTINS: Lazy<IndexMap<&'static Eapi, IndexSet<String>>> =
    Lazy::new(|| {
        EAPIS
            .iter()
            .map(|&e| {
                let builtins = BUILTINS
                    .iter()
                    .filter(|b| b.is_enabled(e) && !b.is_phase())
                    .map(|b| b.to_string())
                    .collect();
                (e, builtins)
            })
            .collect()
    });

/// Controls the status set by the nonfatal builtin.
static NONFATAL: AtomicBool = AtomicBool::new(false);

peg::parser! {
    grammar cmd() for str {
        rule version_separator() -> &'input str
            = s:$([^ 'a'..='z' | 'A'..='Z' | '0'..='9']+) { s }

        rule version_component() -> &'input str
            = s:$(['0'..='9']+) { s }
            / s:$(['a'..='z' | 'A'..='Z']+) { s }

        rule version_element() -> [&'input str; 2]
            = sep:version_separator() comp:version_component()?
            { [sep, comp.unwrap_or_default()] }
            / sep:version_separator()? comp:version_component()
            { [sep.unwrap_or_default(), comp] }

        // Split version strings for ver_rs and ver_cut.
        pub(super) rule version_split() -> Vec<&'input str>
            = vals:version_element()* { vals.into_iter().flatten().collect() }

        // Parse ranges for ver_rs and ver_cut.
        pub(super) rule range(max: usize) -> (usize, usize)
            = start_s:$(['0'..='9']+) "-" end_s:$(['0'..='9']+) {?
                match (start_s.parse(), end_s.parse()) {
                    (Ok(start), Ok(end)) => Ok((start, end)),
                    _ => Err("range value overflow"),
                }
            } / start_s:$(['0'..='9']+) "-" {?
                match start_s.parse() {
                    Ok(start) if start <= max => Ok((start, max)),
                    Ok(start) => Ok((start, start)),
                    _ => Err("range value overflow"),
                }
            } / start_s:$(['0'..='9']+) {?
                let start = start_s.parse().map_err(|_| "range value overflow")?;
                Ok((start, start))
            }
    }
}

// provide public parsing functionality while converting error types
mod parse {
    use crate::peg::peg_error;
    use crate::Error;

    use super::cmd;

    pub(super) fn version_split(s: &str) -> crate::Result<Vec<&str>> {
        cmd::version_split(s).map_err(|e| peg_error(format!("invalid version string: {s}"), s, e))
    }

    pub(super) fn range(s: &str, max: usize) -> crate::Result<(usize, usize)> {
        let (start, end) =
            cmd::range(s, max).map_err(|e| peg_error(format!("invalid range: {s}"), s, e))?;
        if end < start {
            return Err(Error::InvalidValue(format!(
                "start of range ({start}) is greater than end ({end})",
            )));
        }
        Ok((start, end))
    }
}

/// Handle builtin errors, bailing out when running normally.
pub(crate) fn handle_error<S: AsRef<str>>(cmd: S, err: Error) -> ExecStatus {
    let err = match err {
        Error::Base(s) if !NONFATAL.load(Ordering::Relaxed) => Error::Bail(s),
        _ => err,
    };

    scallop::builtins::handle_error(cmd, err)
}

/// Create C compatible builtin function wrapper converting between rust and C types.
#[macro_export]
macro_rules! make_builtin {
    ($name:expr, $func_name:ident, $func:expr, $long_doc:expr, $usage:expr, $scope:expr) => {
        make_builtin!($name, $func_name, $func, $long_doc, $usage, $scope, None, BUILTIN);
    };
    ($name:expr, $func_name:ident, $func:expr, $long_doc:expr, $usage:expr, $scope:expr, $deprecated:expr) => {
        make_builtin!($name, $func_name, $func, $long_doc, $usage, $scope, $deprecated, BUILTIN);
    };
    ($name:expr, $func_name:ident, $func:expr, $long_doc:expr, $usage:expr, $scope:expr, $deprecated:expr, $builtin:ident) => {
        #[no_mangle]
        extern "C" fn $func_name(list: *mut scallop::bash::WordList) -> std::ffi::c_int {
            use scallop::traits::IntoWords;

            use $crate::shell::builtins::{handle_error, BUILTINS};

            let words = list.into_words(false);
            let args: Vec<_> = words.into_iter().collect();
            let builtin = BUILTINS
                .get($name)
                .unwrap_or_else(|| panic!("unregistered builtin: {}", $name));

            let ret = builtin
                .run(&args)
                .unwrap_or_else(|e| handle_error(builtin, e));

            i32::from(ret)
        }

        pub(super) static $builtin: once_cell::sync::Lazy<$crate::shell::builtins::Builtin> =
            once_cell::sync::Lazy::new(|| {
                let builtin = scallop::builtins::Builtin {
                    name: $name,
                    func: $func,
                    flags: scallop::builtins::Attr::ENABLED.bits(),
                    cfunc: $func_name,
                    help: $long_doc,
                    usage: $usage,
                };

                $crate::shell::builtins::Builtin::new(builtin, $scope, $deprecated)
            });
    };
}
use make_builtin;

#[cfg(test)]
fn assert_invalid_args(func: scallop::builtins::BuiltinFn, nums: &[u32]) {
    for n in nums {
        let args: Vec<_> = (0..*n).map(|n| n.to_string()).collect();
        let args: Vec<_> = args.iter().map(|s| s.as_str()).collect();
        let re = format!("^.*, got {n}");
        crate::macros::assert_err_re!(func(&args), re);
    }
}

#[cfg(test)]
macro_rules! builtin_scope_tests {
    ($cmd:expr) => {
        #[test]
        fn test_builtin_scope() {
            use crate::config::Config;
            use crate::eapi::EAPIS_OFFICIAL;
            use crate::macros::assert_err_re;
            use crate::pkg::SourcePackage;
            use crate::shell::builtins::BUILTINS;
            use crate::shell::scope::Scope::*;

            let cmd = $cmd;
            let name = cmd.split(' ').next().unwrap();
            let builtin = BUILTINS.get(name).unwrap();
            let mut config = Config::default();
            let t = config.temp_repo("test", 0, None).unwrap();

            for eapi in &*EAPIS_OFFICIAL {
                let scopes = [Global, Eclass]
                    .into_iter()
                    .chain(eapi.phases().iter().map(Into::into))
                    .filter(|s| {
                        !builtin
                            .scope
                            .get(eapi)
                            .map(|set| set.contains(s))
                            .unwrap_or_default()
                    });
                for scope in scopes {
                    let err = format!("{name}: error: disabled in ");
                    let info = format!("EAPI={eapi}, scope: {scope}");

                    match scope {
                        Eclass => {
                            // create eclass
                            let eclass = indoc::formatdoc! {r#"
                                # stub eclass
                                VAR=1
                                {cmd}
                                VAR=2
                            "#};
                            t.create_eclass("e1", &eclass).unwrap();
                            let data = indoc::formatdoc! {r#"
                                EAPI={eapi}
                                inherit e1
                                DESCRIPTION="testing builtin eclass scope failures"
                                SLOT=0
                            "#};
                            let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
                            let r = raw_pkg.source();
                            // verify sourcing stops at unknown command
                            assert_eq!(scallop::variables::optional("VAR").unwrap(), "1");
                            // verify error output
                            assert_err_re!(r, err, &info);
                        }
                        Global => {
                            let data = indoc::formatdoc! {r#"
                                EAPI={eapi}
                                DESCRIPTION="testing builtin global scope failures"
                                SLOT=0
                                LICENSE="MIT"
                                VAR=1
                                {cmd}
                                VAR=2
                            "#};
                            let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
                            let r = raw_pkg.source();
                            // verify sourcing stops at unknown command
                            assert_eq!(scallop::variables::optional("VAR").unwrap(), "1");
                            // verify error output
                            assert_err_re!(r, err, &info);
                        }
                        Phase(phase) => {
                            let data = indoc::formatdoc! {r#"
                                EAPI={eapi}
                                DESCRIPTION="testing builtin phase scope failures"
                                SLOT=0
                                VAR=1
                                {phase}() {{
                                    {cmd}
                                    VAR=2
                                }}
                            "#};
                            let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                            pkg.source().unwrap();
                            let phase = eapi.phases().get(&phase).unwrap();
                            let r = phase.run();
                            // verify function stops at unknown command
                            assert_eq!(scallop::variables::optional("VAR").as_deref(), Some("1"));
                            // verify error output
                            assert_err_re!(r, err, &info);
                        }
                    }
                }
            }
        }
    };
}
#[cfg(test)]
use builtin_scope_tests;
