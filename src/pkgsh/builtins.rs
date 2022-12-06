use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::AtomicBool;

use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;
use scallop::builtins::{Builtin, ExecStatus};

use crate::{eapi, eapi::Eapi};

use super::phase::Phase;

mod _default_phase_func;
mod _new;
mod _use_conf;
pub(super) mod adddeny;
pub(super) mod addpredict;
pub(super) mod addread;
pub(super) mod addwrite;
pub(super) mod assert;
pub(super) mod best_version;
pub(super) mod command_not_found_handle;
pub(super) mod debug_print;
pub(super) mod debug_print_function;
pub(super) mod debug_print_section;
pub(super) mod default;
pub(super) mod default_pkg_nofetch;
pub(super) mod default_src_compile;
pub(super) mod default_src_configure;
pub(super) mod default_src_install;
pub(super) mod default_src_prepare;
pub(super) mod default_src_test;
pub(super) mod default_src_unpack;
pub(super) mod die;
pub(super) mod diropts;
pub(super) mod dobin;
pub(super) mod docinto;
pub(super) mod docompress;
pub(super) mod doconfd;
pub(super) mod dodir;
pub(super) mod dodoc;
pub(super) mod doenvd;
pub(super) mod doexe;
pub(super) mod dohard;
pub(super) mod doheader;
pub(super) mod dohtml;
pub(super) mod doinfo;
pub(super) mod doinitd;
pub(super) mod doins;
pub(super) mod dolib;
pub(super) mod dolib_a;
pub(super) mod dolib_so;
pub(super) mod doman;
pub(super) mod domo;
pub(super) mod dosbin;
pub(super) mod dosed;
pub(super) mod dostrip;
pub(super) mod dosym;
pub(super) mod eapply;
pub(super) mod eapply_user;
pub(super) mod ebegin;
pub(super) mod econf;
pub(super) mod eend;
pub(super) mod eerror;
pub(super) mod einfo;
pub(super) mod einfon;
pub(super) mod einstall;
pub(super) mod einstalldocs;
pub(super) mod emake;
pub(super) mod eqawarn;
pub(super) mod ewarn;
pub(super) mod exeinto;
pub(super) mod exeopts;
pub(super) mod export_functions;
pub(super) mod fowners;
pub(super) mod fperms;
pub(super) mod get_libdir;
pub(super) mod has;
pub(super) mod has_version;
pub(super) mod hasq;
pub(super) mod hasv;
pub(super) mod in_iuse;
pub(super) mod inherit;
pub(super) mod insinto;
pub(super) mod insopts;
pub(super) mod into;
pub(super) mod keepdir;
pub(super) mod libopts;
pub(super) mod newbin;
pub(super) mod newconfd;
pub(super) mod newdoc;
pub(super) mod newenvd;
pub(super) mod newexe;
pub(super) mod newheader;
pub(super) mod newinitd;
pub(super) mod newins;
pub(super) mod newlib_a;
pub(super) mod newlib_so;
pub(super) mod newman;
pub(super) mod newsbin;
pub(super) mod nonfatal;
pub(super) mod unpack;
pub(super) mod use_;
pub(super) mod use_enable;
pub(super) mod use_with;
pub(super) mod useq;
pub(super) mod usev;
pub(super) mod usex;
pub(super) mod ver_cut;
pub(super) mod ver_rs;
pub(super) mod ver_test;

#[derive(Debug)]
pub(crate) struct PkgBuiltin {
    builtin: Builtin,
    scope: IndexMap<&'static Eapi, Regex>,
}

impl From<&PkgBuiltin> for Builtin {
    fn from(b: &PkgBuiltin) -> Self {
        b.builtin
    }
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
pub(crate) enum Scope {
    #[default]
    Global,
    Eclass,
    Phase(Phase),
}

#[derive(Debug, Clone)]
pub(crate) struct Scopes(Regex);

impl Scopes {
    pub(crate) fn new(scopes: &[&str]) -> Self {
        let s = scopes.join("|");
        Self(Regex::new(&format!(r"^{s}$")).unwrap_or_else(|e| panic!("{e}")))
    }

    pub(crate) fn matches(&self, scope: Scope) -> bool {
        self.0.is_match(scope.as_ref())
    }
}

impl<T: Borrow<Phase>> From<T> for Scope {
    fn from(phase: T) -> Self {
        Scope::Phase(*phase.borrow())
    }
}

impl AsRef<str> for Scope {
    fn as_ref(&self) -> &str {
        match self {
            Self::Eclass => "eclass",
            Self::Global => "global",
            Self::Phase(p) => p.as_ref(),
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

// scope patterns
pub(crate) const ALL: &str = ".+";
pub(crate) const ECLASS: &str = "eclass";
pub(crate) const GLOBAL: &str = "global";
pub(crate) const PHASE: &str = ".+_.+";
pub(crate) const SRC: &str = "src_.+";
pub(crate) const PKG: &str = "pkg_.+";

impl PkgBuiltin {
    fn new(builtin: Builtin, scopes: &[(&str, &[&str])]) -> Self {
        let mut scope = IndexMap::new();
        for (range, s) in scopes.iter() {
            let scope_re = Regex::new(&format!(r"^{}$", s.join("|"))).unwrap();
            let eapis = eapi::range(range).unwrap_or_else(|e| {
                panic!("failed to parse EAPI range for {builtin} builtin: {range}: {e}")
            });
            for e in eapis {
                if scope.insert(e, scope_re.clone()).is_some() {
                    panic!("clashing EAPI scopes: {e}");
                }
            }
        }
        PkgBuiltin { builtin, scope }
    }

    pub(crate) fn run(&self, args: &[&str]) -> scallop::Result<ExecStatus> {
        self.builtin.run(args)
    }

    pub(crate) fn name(&self) -> &'static str {
        self.builtin.name
    }
}

pub(crate) type BuiltinsMap = HashMap<&'static str, &'static PkgBuiltin>;
pub(crate) type ScopeBuiltinsMap = HashMap<Scope, BuiltinsMap>;
pub(crate) type EapiBuiltinsMap = HashMap<&'static Eapi, ScopeBuiltinsMap>;

pub(crate) static ALL_BUILTINS: Lazy<HashMap<&'static str, &PkgBuiltin>> = Lazy::new(|| {
    [
        &*adddeny::PKG_BUILTIN,
        &*addpredict::PKG_BUILTIN,
        &*addread::PKG_BUILTIN,
        &*addwrite::PKG_BUILTIN,
        &*assert::PKG_BUILTIN,
        &*best_version::PKG_BUILTIN,
        &*command_not_found_handle::PKG_BUILTIN,
        &*debug_print::PKG_BUILTIN,
        &*debug_print_function::PKG_BUILTIN,
        &*debug_print_section::PKG_BUILTIN,
        &*default::PKG_BUILTIN,
        &*default_pkg_nofetch::PKG_BUILTIN,
        &*default_src_compile::PKG_BUILTIN,
        &*default_src_configure::PKG_BUILTIN,
        &*default_src_install::PKG_BUILTIN,
        &*default_src_prepare::PKG_BUILTIN,
        &*default_src_test::PKG_BUILTIN,
        &*default_src_unpack::PKG_BUILTIN,
        &*die::PKG_BUILTIN,
        &*diropts::PKG_BUILTIN,
        &*dobin::PKG_BUILTIN,
        &*docinto::PKG_BUILTIN,
        &*docompress::PKG_BUILTIN,
        &*doconfd::PKG_BUILTIN,
        &*dodir::PKG_BUILTIN,
        &*dodoc::PKG_BUILTIN,
        &*doenvd::PKG_BUILTIN,
        &*doexe::PKG_BUILTIN,
        &*dohard::PKG_BUILTIN,
        &*doheader::PKG_BUILTIN,
        &*dohtml::PKG_BUILTIN,
        &*doinfo::PKG_BUILTIN,
        &*doinitd::PKG_BUILTIN,
        &*doins::PKG_BUILTIN,
        &*dolib::PKG_BUILTIN,
        &*dolib_a::PKG_BUILTIN,
        &*dolib_so::PKG_BUILTIN,
        &*doman::PKG_BUILTIN,
        &*domo::PKG_BUILTIN,
        &*dosbin::PKG_BUILTIN,
        &*dosed::PKG_BUILTIN,
        &*dostrip::PKG_BUILTIN,
        &*dosym::PKG_BUILTIN,
        &*eapply::PKG_BUILTIN,
        &*eapply_user::PKG_BUILTIN,
        &*ebegin::PKG_BUILTIN,
        &*econf::PKG_BUILTIN,
        &*eend::PKG_BUILTIN,
        &*eerror::PKG_BUILTIN,
        &*einfo::PKG_BUILTIN,
        &*einfon::PKG_BUILTIN,
        &*einstall::PKG_BUILTIN,
        &*einstalldocs::PKG_BUILTIN,
        &*emake::PKG_BUILTIN,
        &*eqawarn::PKG_BUILTIN,
        &*ewarn::PKG_BUILTIN,
        &*exeinto::PKG_BUILTIN,
        &*exeopts::PKG_BUILTIN,
        &*export_functions::PKG_BUILTIN,
        &*fowners::PKG_BUILTIN,
        &*fperms::PKG_BUILTIN,
        &*get_libdir::PKG_BUILTIN,
        &*has::PKG_BUILTIN,
        &*has_version::PKG_BUILTIN,
        &*hasq::PKG_BUILTIN,
        &*hasv::PKG_BUILTIN,
        &*in_iuse::PKG_BUILTIN,
        &*inherit::PKG_BUILTIN,
        &*insinto::PKG_BUILTIN,
        &*insopts::PKG_BUILTIN,
        &*into::PKG_BUILTIN,
        &*keepdir::PKG_BUILTIN,
        &*libopts::PKG_BUILTIN,
        &*newbin::PKG_BUILTIN,
        &*newconfd::PKG_BUILTIN,
        &*newdoc::PKG_BUILTIN,
        &*newenvd::PKG_BUILTIN,
        &*newexe::PKG_BUILTIN,
        &*newheader::PKG_BUILTIN,
        &*newinitd::PKG_BUILTIN,
        &*newins::PKG_BUILTIN,
        &*newlib_a::PKG_BUILTIN,
        &*newlib_so::PKG_BUILTIN,
        &*newman::PKG_BUILTIN,
        &*newsbin::PKG_BUILTIN,
        &*nonfatal::PKG_BUILTIN,
        &*unpack::PKG_BUILTIN,
        &*use_::PKG_BUILTIN,
        &*use_enable::PKG_BUILTIN,
        &*use_with::PKG_BUILTIN,
        &*useq::PKG_BUILTIN,
        &*usev::PKG_BUILTIN,
        &*usex::PKG_BUILTIN,
        &*ver_cut::PKG_BUILTIN,
        &*ver_rs::PKG_BUILTIN,
        &*ver_test::PKG_BUILTIN,
    ]
    .into_iter()
    .map(|b| (b.name(), b))
    .collect()
});

// TODO: auto-generate the builtin module imports and vector creation via build script
pub(crate) static BUILTINS_MAP: Lazy<EapiBuiltinsMap> = Lazy::new(|| {
    let static_scopes: Vec<_> = vec![Scope::Global, Scope::Eclass];
    #[allow(clippy::mutable_key_type)]
    let mut builtins_map = EapiBuiltinsMap::new();
    for b in ALL_BUILTINS.values() {
        for (eapi, re) in b.scope.iter() {
            let scope_map = builtins_map
                .entry(eapi)
                .or_insert_with(ScopeBuiltinsMap::new);
            let phase_scopes: Vec<_> = eapi.phases().iter().map(|p| p.into()).collect();
            let scopes = static_scopes.iter().chain(phase_scopes.iter());
            for scope in scopes.filter(|s| re.is_match(s.as_ref())) {
                scope_map
                    .entry(*scope)
                    .or_insert_with(BuiltinsMap::new)
                    .insert(b.name(), b);
            }
        }
    }
    builtins_map
});

static NONFATAL: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

static VERSION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?P<sep>[[:^alnum:]]+)?(?P<comp>[[:digit:]]+|[[:alpha:]]+)?").unwrap()
});

/// Split version string into a vector of separators and components.
fn version_split(ver: &str) -> Vec<&str> {
    let mut version_parts = Vec::new();
    for caps in VERSION_RE.captures_iter(ver) {
        version_parts.extend([
            caps.name("sep").map_or("", |m| m.as_str()),
            caps.name("comp").map_or("", |m| m.as_str()),
        ]);
    }
    version_parts
}

peg::parser! {
    grammar cmd() for str {
        // Parse ranges used with the ver_rs and ver_cut commands.
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

    pub(crate) fn range(s: &str, max: usize) -> crate::Result<(usize, usize)> {
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

/// Create C compatible builtin function wrapper converting between rust and C types.
#[macro_export]
macro_rules! make_builtin {
    ($name:expr, $func_name:ident, $func:expr, $long_doc:expr, $usage:expr, $scope:expr) => {
        use std::ffi::c_int;

        use once_cell::sync::Lazy;
        use scallop::builtins::Builtin;
        use scallop::traits::IntoWords;

        use $crate::pkgsh::builtins::{PkgBuiltin, ALL_BUILTINS};

        #[no_mangle]
        extern "C" fn $func_name(list: *mut scallop::bash::WordList) -> c_int {
            let words = list.into_words(false);
            let args: Vec<_> = words.into_iter().collect();

            let run_builtin = || -> ExecStatus {
                $crate::pkgsh::BUILD_DATA.with(|d| {
                    let cmd = $name;
                    let scope = d.borrow().scope;
                    let eapi = d.borrow().eapi;

                    if eapi.builtins(scope).contains_key(cmd) {
                        match $func(&args) {
                            Ok(ret) => ret,
                            Err(e) => scallop::builtins::handle_error(cmd, e),
                        }
                    } else {
                        let builtin = ALL_BUILTINS.get(cmd).expect("unknown builtin: {cmd}");
                        let msg = match builtin.scope.get(eapi) {
                            Some(_) => format!("{scope} scope doesn't enable command: {cmd}"),
                            None => format!("EAPI={eapi} doesn't enable command: {cmd}"),
                        };
                        scallop::builtins::handle_error(cmd, scallop::Error::Base(msg))
                    }
                })
            };

            i32::from(run_builtin())
        }

        pub(super) static BUILTIN: Builtin = Builtin {
            name: $name,
            func: $func,
            cfunc: $func_name,
            help: $long_doc,
            usage: $usage,
        };

        pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
            Lazy::new(|| PkgBuiltin::new(BUILTIN, $scope));
    };
}
pub(self) use make_builtin;

#[cfg(test)]
fn assert_invalid_args(func: ::scallop::builtins::BuiltinFn, nums: &[u32]) {
    for n in nums {
        let args: Vec<String> = (0..*n).map(|n| n.to_string()).collect();
        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
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
            use crate::pkgsh::{
                builtins::Scope::*, run_phase, source_ebuild, BuildData, BUILD_DATA,
            };

            let cmd = $cmd;
            let name = cmd.split(' ').next().unwrap();
            let mut config = Config::new("pkgcraft", "", false).unwrap();
            let (t, repo) = config.temp_repo("test", 0).unwrap();
            let (_, cpv) = t.create_ebuild("cat/pkg-1", []).unwrap();
            BuildData::update(&cpv, &repo);

            let static_scopes: Vec<_> = vec![Global, Eclass];
            for eapi in EAPIS_OFFICIAL.iter() {
                let phase_scopes: Vec<_> = eapi.phases().iter().map(|p| p.into()).collect();
                let scopes = static_scopes.iter().chain(phase_scopes.iter());
                for scope in scopes.filter(|&s| !eapi.builtins(*s).contains_key(name)) {
                    let err = format!(" doesn't enable command: {name}");
                    let info = format!("EAPI={eapi}, scope: {scope}");

                    // initialize build state
                    BUILD_DATA.with(|d| d.borrow_mut().eapi = eapi);

                    match scope {
                        Eclass => {
                            // create eclass
                            let eclass = indoc::formatdoc! {r#"
                                # stub eclass
                                {cmd}
                            "#};
                            t.create_eclass("e1", &eclass).unwrap();
                            let data = indoc::formatdoc! {r#"
                                EAPI={eapi}
                                inherit e1
                                DESCRIPTION="testing builtin eclass scope failures"
                                SLOT=0
                            "#};
                            let (path, _) = t.create_ebuild_raw("cat/pkg-1", &data).unwrap();
                            let r = source_ebuild(&path);
                            assert_err_re!(r, err, &info);
                        }
                        Global => {
                            let data = indoc::formatdoc! {r#"
                                EAPI={eapi}
                                DESCRIPTION="testing builtin global scope failures"
                                SLOT=0
                                {cmd}
                            "#};
                            let (path, _) = t.create_ebuild_raw("cat/pkg-1", &data).unwrap();
                            let r = source_ebuild(&path);
                            assert_err_re!(r, err, &info);
                        }
                        Phase(phase) => {
                            let data = indoc::formatdoc! {r#"
                                EAPI={eapi}
                                DESCRIPTION="testing builtin phase scope failures"
                                SLOT=0
                                {phase}() {{
                                    {cmd}
                                }}
                            "#};
                            let (path, _) = t.create_ebuild_raw("cat/pkg-1", &data).unwrap();
                            source_ebuild(&path).unwrap();
                            let r = run_phase(*phase);
                            assert_err_re!(r, err, &info);
                        }
                    }
                }
            }
        }
    };
}
#[cfg(test)]
pub(self) use builtin_scope_tests;
