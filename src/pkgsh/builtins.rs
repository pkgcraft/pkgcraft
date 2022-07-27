use std::borrow::Borrow;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;
use scallop::builtins::{Builtin, ExecStatus};

use super::phase::Phase;
use crate::{eapi, eapi::Eapi};

mod _default_phase_func;
mod _new;
mod _use_conf;
pub mod adddeny;
pub mod addpredict;
pub mod addread;
pub mod addwrite;
pub mod assert;
pub mod debug_print;
pub mod debug_print_function;
pub mod debug_print_section;
pub mod default;
pub mod default_pkg_nofetch;
pub mod default_src_compile;
pub mod default_src_configure;
pub mod default_src_install;
pub mod default_src_prepare;
pub mod default_src_test;
pub mod default_src_unpack;
pub mod die;
pub mod diropts;
pub mod dobin;
pub mod docinto;
pub mod docompress;
pub mod doconfd;
pub mod dodir;
pub mod dodoc;
pub mod doenvd;
pub mod doexe;
pub mod dohard;
pub mod doheader;
pub mod dohtml;
pub mod doinfo;
pub mod doinitd;
pub mod doins;
pub mod dolib;
pub mod dolib_a;
pub mod dolib_so;
pub mod doman;
pub mod domo;
pub mod dosbin;
pub mod dosed;
pub mod dostrip;
pub mod dosym;
pub mod eapply;
pub mod eapply_user;
pub mod ebegin;
pub mod econf;
pub mod eend;
pub mod eerror;
pub mod einfo;
pub mod einfon;
pub mod einstall;
pub mod einstalldocs;
pub mod emake;
pub mod eqawarn;
pub mod ewarn;
pub mod exeinto;
pub mod exeopts;
pub mod export_functions;
pub mod fowners;
pub mod fperms;
pub mod get_libdir;
pub mod has;
pub mod hasq;
pub mod hasv;
pub mod in_iuse;
pub mod inherit;
pub mod insinto;
pub mod insopts;
pub mod into;
pub mod keepdir;
pub mod libopts;
pub mod newbin;
pub mod newconfd;
pub mod newdoc;
pub mod newenvd;
pub mod newexe;
pub mod newheader;
pub mod newinitd;
pub mod newins;
pub mod newlib_a;
pub mod newlib_so;
pub mod newman;
pub mod newsbin;
pub mod nonfatal;
pub mod unpack;
pub mod use_;
pub mod use_enable;
pub mod use_with;
pub mod useq;
pub mod usev;
pub mod usex;
pub mod ver_cut;
pub mod ver_rs;
pub mod ver_test;

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

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub(crate) enum Scope {
    Eclass,
    Global,
    Phase(Phase),
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

// scope patterns
const ALL: &str = ".+";
const ECLASS: &str = "eclass";
const GLOBAL: &str = "global";
const PHASE: &str = ".+_.+";

impl PkgBuiltin {
    fn new(builtin: Builtin, scopes: &[(&str, &[&str])]) -> Self {
        let mut scope = IndexMap::new();
        for (eapis, s) in scopes.iter() {
            let scope_re = Regex::new(&format!(r"^{}$", s.join("|"))).unwrap();
            for e in eapi::supported(eapis)
                .unwrap_or_else(|_| panic!("failed to parse {builtin} EAPI range: {eapis}"))
            {
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
}

pub(crate) type BuiltinsMap = HashMap<&'static str, &'static PkgBuiltin>;
pub(crate) type ScopeBuiltinsMap = HashMap<Scope, BuiltinsMap>;
pub(crate) type EapiBuiltinsMap = HashMap<&'static Eapi, ScopeBuiltinsMap>;

pub(crate) static ALL_BUILTINS: Lazy<Vec<&PkgBuiltin>> = Lazy::new(|| {
    [
        &*adddeny::PKG_BUILTIN,
        &*addpredict::PKG_BUILTIN,
        &*addread::PKG_BUILTIN,
        &*addwrite::PKG_BUILTIN,
        &*assert::PKG_BUILTIN,
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
    .collect()
});

// TODO: auto-generate the builtin module imports and vector creation via build script
pub(crate) static BUILTINS_MAP: Lazy<EapiBuiltinsMap> = Lazy::new(|| {
    let static_scopes: Vec<_> = vec![Scope::Global, Scope::Eclass];
    #[allow(clippy::mutable_key_type)]
    let mut builtins_map = EapiBuiltinsMap::new();
    for b in ALL_BUILTINS.iter() {
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
                    .insert(b.builtin.name, b);
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
        pub rule range(max: usize) -> (usize, usize)
            = start_s:$(['0'..='9']+) "-" end_s:$(['0'..='9']+) {
                let start = start_s.parse::<usize>().unwrap();
                let end = end_s.parse::<usize>().unwrap();
                (start, end)
            } / start_s:$(['0'..='9']+) "-" {
                match start_s.parse::<usize>().unwrap() {
                    start if start <= max => (start, max),
                    start => (start, start),
                }
            } / start_s:$(['0'..='9']+) {
                let start = start_s.parse::<usize>().unwrap();
                (start, start)
            }
    }
}

// provide public parsing functionality while converting error types
pub(crate) mod parse {
    use crate::peg::peg_error;

    use super::cmd;
    use crate::{Error, Result};

    pub(crate) fn range(s: &str, max: usize) -> Result<(usize, usize)> {
        let (start, end) =
            cmd::range(s, max).map_err(|e| peg_error(format!("invalid range: {s:?}"), s, e))?;
        if end < start {
            return Err(Error::InvalidValue(format!(
                "start of range ({start}) is greater than end ({end})",
            )));
        }
        Ok((start, end))
    }
}

#[cfg(test)]
fn assert_invalid_args(func: ::scallop::builtins::BuiltinFn, nums: &[u32]) {
    for n in nums {
        let args: Vec<String> = (0..*n).map(|n| n.to_string()).collect();
        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let re = format!("^.*, got {n}");
        crate::macros::assert_err_re!(func(&args), re);
    }
}
