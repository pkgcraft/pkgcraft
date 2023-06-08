use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::AtomicBool;

use indexmap::IndexMap;
use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use strum::IntoEnumIterator;

use crate::{eapi, eapi::Eapi};

use super::phase::{Phase, PhaseKind};

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
pub(super) mod elog;
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
    scope: IndexMap<&'static Eapi, HashSet<Scope>>,
}

impl From<&PkgBuiltin> for Builtin {
    fn from(b: &PkgBuiltin) -> Self {
        b.builtin
    }
}

impl PkgBuiltin {
    fn new(builtin: Builtin, scopes: &[(&str, &[Scopes])]) -> Self {
        let mut scope = IndexMap::new();
        for (range, scopes) in scopes.iter() {
            let scopes: HashSet<_> = scopes.iter().flat_map(|s| s.iter()).collect();
            let eapis = eapi::range(range).unwrap_or_else(|e| {
                panic!("failed to parse EAPI range for {builtin} builtin: {range}: {e}")
            });
            for e in eapis {
                if scope.insert(e, scopes.clone()).is_some() {
                    panic!("clashing EAPI scopes: {e}");
                }
            }
        }
        PkgBuiltin { builtin, scope }
    }

    /// Run a builtin if it's enabled for the current build state.
    pub(crate) fn run(&self, args: &[&str]) -> scallop::Result<ExecStatus> {
        if self.enabled() {
            self.builtin.run(args)
        } else {
            let build = crate::pkgsh::get_build_mut();
            let eapi = build.eapi();
            let scope = &build.scope;
            let msg = match self.scope.get(eapi) {
                Some(_) => format!("{scope} scope doesn't enable command: {self}"),
                None => format!("EAPI={eapi} doesn't enable command: {self}"),
            };
            Err(scallop::Error::Bail(msg))
        }
    }

    /// Check if a builtin is enabled for the current build state.
    pub(crate) fn enabled(&self) -> bool {
        let build = crate::pkgsh::get_build_mut();
        let eapi = build.eapi();
        let scope = &build.scope;

        self.scope
            .get(eapi)
            .map(|s| s.contains(scope))
            .unwrap_or_default()
    }
}

impl AsRef<str> for PkgBuiltin {
    fn as_ref(&self) -> &str {
        self.builtin.name
    }
}

impl Eq for PkgBuiltin {}

impl PartialEq for PkgBuiltin {
    fn eq(&self, other: &Self) -> bool {
        self.builtin.name == other.builtin.name
    }
}

impl Hash for PkgBuiltin {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.builtin.name.hash(state);
    }
}

impl Ord for PkgBuiltin {
    fn cmp(&self, other: &Self) -> Ordering {
        self.builtin.name.cmp(other.builtin.name)
    }
}

impl PartialOrd for PkgBuiltin {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Borrow<str> for &PkgBuiltin {
    fn borrow(&self) -> &str {
        self.builtin.name
    }
}

impl fmt::Display for PkgBuiltin {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd, Copy, Clone)]
pub(crate) enum Scope {
    #[default]
    Global,
    Eclass,
    Phase(PhaseKind),
}

impl<T: Borrow<Phase>> From<T> for Scope {
    fn from(phase: T) -> Self {
        Scope::Phase(phase.borrow().into())
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

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub(crate) enum Scopes {
    All,
    Eclass,
    Global,
    Phases,
    Src,
    Pkg,
    Phase(PhaseKind),
}

impl Scopes {
    /// Convert a scopes identifier into an iterable of [`Scope`] objects.
    pub(crate) fn iter(&self) -> Box<dyn Iterator<Item = Scope>> {
        use Scopes::*;
        match self {
            Eclass => Box::new([Scope::Eclass].into_iter()),
            Global => Box::new([Scope::Global].into_iter()),
            Phases => Box::new(PhaseKind::iter().map(Scope::Phase)),
            Src => Box::new(Phases.iter().filter(|k| k.as_ref().starts_with("src_"))),
            Pkg => Box::new(Phases.iter().filter(|k| k.as_ref().starts_with("pkg_"))),
            All => Box::new([Global, Eclass, Phases].iter().flat_map(|s| s.iter())),
            Phase(p) => Box::new([Scope::Phase(*p)].into_iter()),
        }
    }
}

pub(crate) static BUILTINS: Lazy<HashSet<&PkgBuiltin>> = Lazy::new(|| {
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
        &*elog::PKG_BUILTIN,
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

/// Create C compatible builtin function wrapper converting between rust and C types.
#[macro_export]
macro_rules! make_builtin {
    ($name:expr, $func_name:ident, $func:expr, $long_doc:expr, $usage:expr, $scope:expr) => {
        use std::ffi::c_int;

        use once_cell::sync::Lazy;
        use scallop::builtins::Builtin;

        use $crate::pkgsh::builtins::PkgBuiltin;

        #[no_mangle]
        extern "C" fn $func_name(list: *mut scallop::bash::WordList) -> c_int {
            use scallop::builtins::handle_error;
            use scallop::traits::IntoWords;
            use $crate::pkgsh::builtins::BUILTINS;

            let words = list.into_words(false);
            let args: Vec<_> = words.into_iter().collect();
            let builtin = BUILTINS.get($name).expect("unregistered builtin");

            let ret = match builtin.run(&args) {
                Ok(ret) => ret,
                Err(e) => handle_error(builtin, e),
            };

            i32::from(ret)
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
            use crate::pkg::SourceablePackage;
            use crate::pkgsh::builtins::{Scope::*, BUILTINS};
            use crate::pkgsh::{get_build_mut, BuildData};

            let cmd = $cmd;
            let name = cmd.split(' ').next().unwrap();
            let builtin = BUILTINS.get(name).unwrap();
            let mut config = Config::default();
            let t = config.temp_repo("test", 0, None).unwrap();

            let static_scopes: Vec<_> = vec![Global, Eclass];
            for eapi in EAPIS_OFFICIAL.iter() {
                let phase_scopes: Vec<_> = eapi.phases().iter().map(|p| p.into()).collect();
                let scopes = static_scopes
                    .iter()
                    .chain(phase_scopes.iter())
                    .filter(|&s| {
                        !builtin
                            .scope
                            .get(eapi)
                            .map(|set| set.contains(s))
                            .unwrap_or_default()
                    });
                for scope in scopes {
                    let err = format!(" doesn't enable command: {name}");
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
                            let raw_pkg = t.create_ebuild_raw("cat/pkg-1", &data).unwrap();
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
                            let raw_pkg = t.create_ebuild_raw("cat/pkg-1", &data).unwrap();
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
                                {phase}() {{
                                    local VAR=1
                                    {cmd}
                                    VAR=2
                                }}
                            "#};
                            let raw_pkg = t.create_ebuild_raw("cat/pkg-1", &data).unwrap();
                            let pkg = raw_pkg.into_pkg().unwrap();
                            BuildData::from_pkg(&pkg);
                            get_build_mut().source_ebuild(&pkg.abspath()).unwrap();
                            let phase = eapi.phases().get(phase).unwrap();
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
pub(self) use builtin_scope_tests;
