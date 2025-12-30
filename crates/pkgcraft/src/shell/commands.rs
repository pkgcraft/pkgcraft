use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::FromStr;
use std::sync::LazyLock;
use std::{cmp, fmt};

use indexmap::IndexSet;

use super::get_build_mut;
use super::phase::PhaseKind;
use super::scope::ScopeSet;

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
mod eapply;
mod eapply_user;
mod ebegin;
pub(crate) mod econf;
mod edo;
mod eend;
mod eerror;
mod einfo;
mod einfon;
mod einstall;
pub(super) mod einstalldocs;
mod elog;
mod emake;
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
mod pipestatus;
mod unpack;
mod use_;
mod use_enable;
mod use_with;
mod useq;
mod usev;
mod usex;
mod ver_cut;
mod ver_replacing;
mod ver_rs;
mod ver_test;

// export command builtins for internal use
pub(crate) mod builtins {
    pub(crate) use super::adddeny::BUILTIN as adddeny;
    pub(crate) use super::addpredict::BUILTIN as addpredict;
    pub(crate) use super::addread::BUILTIN as addread;
    pub(crate) use super::addwrite::BUILTIN as addwrite;
    pub(crate) use super::assert::BUILTIN as assert;
    pub(crate) use super::best_version::BUILTIN as best_version;
    pub(crate) use super::command_not_found_handle::BUILTIN as command_not_found_handle;
    pub(crate) use super::debug_print::BUILTIN as debug_print;
    pub(crate) use super::debug_print_function::BUILTIN as debug_print_function;
    pub(crate) use super::debug_print_section::BUILTIN as debug_print_section;
    pub(crate) use super::default::BUILTIN as default;
    pub(crate) use super::default_pkg_nofetch::BUILTIN as default_pkg_nofetch;
    pub(crate) use super::default_src_compile::BUILTIN as default_src_compile;
    pub(crate) use super::default_src_configure::BUILTIN as default_src_configure;
    pub(crate) use super::default_src_install::BUILTIN as default_src_install;
    pub(crate) use super::default_src_prepare::BUILTIN as default_src_prepare;
    pub(crate) use super::default_src_test::BUILTIN as default_src_test;
    pub(crate) use super::default_src_unpack::BUILTIN as default_src_unpack;
    pub(crate) use super::die::BUILTIN as die;
    pub(crate) use super::diropts::BUILTIN as diropts;
    pub(crate) use super::dobin::BUILTIN as dobin;
    pub(crate) use super::docinto::BUILTIN as docinto;
    pub(crate) use super::docompress::BUILTIN as docompress;
    pub(crate) use super::doconfd::BUILTIN as doconfd;
    pub(crate) use super::dodir::BUILTIN as dodir;
    pub(crate) use super::dodoc::BUILTIN as dodoc;
    pub(crate) use super::doenvd::BUILTIN as doenvd;
    pub(crate) use super::doexe::BUILTIN as doexe;
    pub(crate) use super::doheader::BUILTIN as doheader;
    pub(crate) use super::dohtml::BUILTIN as dohtml;
    pub(crate) use super::doinfo::BUILTIN as doinfo;
    pub(crate) use super::doinitd::BUILTIN as doinitd;
    pub(crate) use super::doins::BUILTIN as doins;
    pub(crate) use super::dolib::BUILTIN as dolib;
    pub(crate) use super::dolib_a::BUILTIN as dolib_a;
    pub(crate) use super::dolib_so::BUILTIN as dolib_so;
    pub(crate) use super::doman::BUILTIN as doman;
    pub(crate) use super::domo::BUILTIN as domo;
    pub(crate) use super::dosbin::BUILTIN as dosbin;
    pub(crate) use super::dostrip::BUILTIN as dostrip;
    pub(crate) use super::dosym::BUILTIN as dosym;
    pub(crate) use super::eapply::BUILTIN as eapply;
    pub(crate) use super::eapply_user::BUILTIN as eapply_user;
    pub(crate) use super::ebegin::BUILTIN as ebegin;
    pub(crate) use super::econf::BUILTIN as econf;
    pub(crate) use super::edo::BUILTIN as edo;
    pub(crate) use super::eend::BUILTIN as eend;
    pub(crate) use super::eerror::BUILTIN as eerror;
    pub(crate) use super::einfo::BUILTIN as einfo;
    pub(crate) use super::einfon::BUILTIN as einfon;
    pub(crate) use super::einstall::BUILTIN as einstall;
    pub(crate) use super::einstalldocs::BUILTIN as einstalldocs;
    pub(crate) use super::elog::BUILTIN as elog;
    pub(crate) use super::emake::BUILTIN as emake;
    pub(crate) use super::eqawarn::BUILTIN as eqawarn;
    pub(crate) use super::ewarn::BUILTIN as ewarn;
    pub(crate) use super::exeinto::BUILTIN as exeinto;
    pub(crate) use super::exeopts::BUILTIN as exeopts;
    pub(crate) use super::export_functions::BUILTIN as export_functions;
    pub(crate) use super::fowners::BUILTIN as fowners;
    pub(crate) use super::fperms::BUILTIN as fperms;
    pub(crate) use super::get_libdir::BUILTIN as get_libdir;
    pub(crate) use super::has::BUILTIN as has;
    pub(crate) use super::has_version::BUILTIN as has_version;
    pub(crate) use super::hasq::BUILTIN as hasq;
    pub(crate) use super::hasv::BUILTIN as hasv;
    pub(crate) use super::in_iuse::BUILTIN as in_iuse;
    pub(crate) use super::inherit::BUILTIN as inherit;
    pub(crate) use super::insinto::BUILTIN as insinto;
    pub(crate) use super::insopts::BUILTIN as insopts;
    pub(crate) use super::into::BUILTIN as into;
    pub(crate) use super::keepdir::BUILTIN as keepdir;
    pub(crate) use super::libopts::BUILTIN as libopts;
    pub(crate) use super::newbin::BUILTIN as newbin;
    pub(crate) use super::newconfd::BUILTIN as newconfd;
    pub(crate) use super::newdoc::BUILTIN as newdoc;
    pub(crate) use super::newenvd::BUILTIN as newenvd;
    pub(crate) use super::newexe::BUILTIN as newexe;
    pub(crate) use super::newheader::BUILTIN as newheader;
    pub(crate) use super::newinitd::BUILTIN as newinitd;
    pub(crate) use super::newins::BUILTIN as newins;
    pub(crate) use super::newlib_a::BUILTIN as newlib_a;
    pub(crate) use super::newlib_so::BUILTIN as newlib_so;
    pub(crate) use super::newman::BUILTIN as newman;
    pub(crate) use super::newsbin::BUILTIN as newsbin;
    pub(crate) use super::nonfatal::BUILTIN as nonfatal;
    pub(crate) use super::pipestatus::BUILTIN as pipestatus;
    pub(crate) use super::unpack::BUILTIN as unpack;
    pub(crate) use super::use_::BUILTIN as use_;
    pub(crate) use super::use_enable::BUILTIN as use_enable;
    pub(crate) use super::use_with::BUILTIN as use_with;
    pub(crate) use super::useq::BUILTIN as useq;
    pub(crate) use super::usev::BUILTIN as usev;
    pub(crate) use super::usex::BUILTIN as usex;
    pub(crate) use super::ver_cut::BUILTIN as ver_cut;
    pub(crate) use super::ver_replacing::BUILTIN as ver_replacing;
    pub(crate) use super::ver_rs::BUILTIN as ver_rs;
    pub(crate) use super::ver_test::BUILTIN as ver_test;
    // phase stubs
    pub(crate) use super::_phases::PKG_CONFIG as pkg_config;
    pub(crate) use super::_phases::PKG_INFO as pkg_info;
    pub(crate) use super::_phases::PKG_NOFETCH as pkg_nofetch;
    pub(crate) use super::_phases::PKG_POSTINST as pkg_postinst;
    pub(crate) use super::_phases::PKG_POSTRM as pkg_postrm;
    pub(crate) use super::_phases::PKG_PREINST as pkg_preinst;
    pub(crate) use super::_phases::PKG_PRERM as pkg_prerm;
    pub(crate) use super::_phases::PKG_PRETEND as pkg_pretend;
    pub(crate) use super::_phases::PKG_SETUP as pkg_setup;
    pub(crate) use super::_phases::SRC_COMPILE as src_compile;
    pub(crate) use super::_phases::SRC_CONFIGURE as src_configure;
    pub(crate) use super::_phases::SRC_INSTALL as src_install;
    pub(crate) use super::_phases::SRC_PREPARE as src_prepare;
    pub(crate) use super::_phases::SRC_TEST as src_test;
    pub(crate) use super::_phases::SRC_UNPACK as src_unpack;
}

// export command functions for internal use
#[allow(unused_imports)]
pub(crate) mod functions {
    pub(crate) use super::adddeny::run as adddeny;
    pub(crate) use super::addpredict::run as addpredict;
    pub(crate) use super::addread::run as addread;
    pub(crate) use super::addwrite::run as addwrite;
    pub(crate) use super::assert::run as assert;
    pub(crate) use super::best_version::run as best_version;
    pub(crate) use super::debug_print::run as debug_print;
    pub(crate) use super::debug_print_function::run as debug_print_function;
    pub(crate) use super::debug_print_section::run as debug_print_section;
    pub(crate) use super::default::run as default;
    pub(crate) use super::default_pkg_nofetch::run as default_pkg_nofetch;
    pub(crate) use super::default_src_compile::run as default_src_compile;
    pub(crate) use super::default_src_configure::run as default_src_configure;
    pub(crate) use super::default_src_install::run as default_src_install;
    pub(crate) use super::default_src_prepare::run as default_src_prepare;
    pub(crate) use super::default_src_test::run as default_src_test;
    pub(crate) use super::default_src_unpack::run as default_src_unpack;
    pub(crate) use super::die::run as die;
    pub(crate) use super::diropts::run as diropts;
    pub(crate) use super::dobin::run as dobin;
    pub(crate) use super::docinto::run as docinto;
    pub(crate) use super::docompress::run as docompress;
    pub(crate) use super::doconfd::run as doconfd;
    pub(crate) use super::dodir::run as dodir;
    pub(crate) use super::dodoc::run as dodoc;
    pub(crate) use super::doenvd::run as doenvd;
    pub(crate) use super::doexe::run as doexe;
    pub(crate) use super::doheader::run as doheader;
    pub(crate) use super::dohtml::run as dohtml;
    pub(crate) use super::doinfo::run as doinfo;
    pub(crate) use super::doinitd::run as doinitd;
    pub(crate) use super::doins::run as doins;
    pub(crate) use super::dolib::run as dolib;
    pub(crate) use super::dolib_a::run as dolib_a;
    pub(crate) use super::dolib_so::run as dolib_so;
    pub(crate) use super::doman::run as doman;
    pub(crate) use super::domo::run as domo;
    pub(crate) use super::dosbin::run as dosbin;
    pub(crate) use super::dostrip::run as dostrip;
    pub(crate) use super::dosym::run as dosym;
    pub(crate) use super::eapply::run as eapply;
    pub(crate) use super::eapply_user::run as eapply_user;
    pub(crate) use super::ebegin::run as ebegin;
    pub(crate) use super::econf::run as econf;
    pub(crate) use super::edo::run as edo;
    pub(crate) use super::eend::run as eend;
    pub(crate) use super::eerror::run as eerror;
    pub(crate) use super::einfo::run as einfo;
    pub(crate) use super::einfon::run as einfon;
    pub(crate) use super::einstall::run as einstall;
    pub(crate) use super::einstalldocs::run as einstalldocs;
    pub(crate) use super::elog::run as elog;
    pub(crate) use super::emake::run as emake;
    pub(crate) use super::eqawarn::run as eqawarn;
    pub(crate) use super::ewarn::run as ewarn;
    pub(crate) use super::exeinto::run as exeinto;
    pub(crate) use super::exeopts::run as exeopts;
    pub(crate) use super::export_functions::run as export_functions;
    pub(crate) use super::fowners::run as fowners;
    pub(crate) use super::fperms::run as fperms;
    pub(crate) use super::get_libdir::run as get_libdir;
    pub(crate) use super::has::run as has;
    pub(crate) use super::has_version::run as has_version;
    pub(crate) use super::hasq::run as hasq;
    pub(crate) use super::hasv::run as hasv;
    pub(crate) use super::in_iuse::run as in_iuse;
    pub(crate) use super::inherit::run as inherit;
    pub(crate) use super::insinto::run as insinto;
    pub(crate) use super::insopts::run as insopts;
    pub(crate) use super::into::run as into;
    pub(crate) use super::keepdir::run as keepdir;
    pub(crate) use super::libopts::run as libopts;
    pub(crate) use super::newbin::run as newbin;
    pub(crate) use super::newconfd::run as newconfd;
    pub(crate) use super::newdoc::run as newdoc;
    pub(crate) use super::newenvd::run as newenvd;
    pub(crate) use super::newexe::run as newexe;
    pub(crate) use super::newheader::run as newheader;
    pub(crate) use super::newinitd::run as newinitd;
    pub(crate) use super::newins::run as newins;
    pub(crate) use super::newlib_a::run as newlib_a;
    pub(crate) use super::newlib_so::run as newlib_so;
    pub(crate) use super::newman::run as newman;
    pub(crate) use super::newsbin::run as newsbin;
    pub(crate) use super::nonfatal::run as nonfatal;
    pub(crate) use super::pipestatus::run as pipestatus;
    pub(crate) use super::unpack::run as unpack;
    pub(crate) use super::use_::run as use_;
    pub(crate) use super::use_enable::run as use_enable;
    pub(crate) use super::use_with::run as use_with;
    pub(crate) use super::useq::run as useq;
    pub(crate) use super::usev::run as usev;
    pub(crate) use super::usex::run as usex;
    pub(crate) use super::ver_cut::run as ver_cut;
    pub(crate) use super::ver_replacing::run as ver_replacing;
    pub(crate) use super::ver_rs::run as ver_rs;
    pub(crate) use super::ver_test::run as ver_test;
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub(crate) struct Builtin(scallop::builtins::Builtin);

impl Builtin {
    pub(crate) fn allowed_in<I>(self, scopes: I) -> Command
    where
        I: IntoIterator,
        I::Item: Into<ScopeSet>,
    {
        Command {
            builtin: self,
            allowed: scopes.into_iter().map(Into::into).collect(),
            die_on_failure: true,
        }
    }
}

impl From<&Builtin> for scallop::builtins::Builtin {
    fn from(value: &Builtin) -> Self {
        value.0
    }
}

impl fmt::Display for Builtin {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// TODO: replace with callable trait implementation if it's ever stabilized
// https://github.com/rust-lang/rust/issues/29625
impl Deref for Builtin {
    type Target = scallop::builtins::Builtin;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct Command {
    builtin: Builtin,
    pub allowed: HashSet<ScopeSet>,
    pub die_on_failure: bool,
}

impl PartialEq for Command {
    fn eq(&self, other: &Self) -> bool {
        self.builtin == other.builtin
    }
}

impl Eq for Command {}

impl Hash for Command {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.builtin.hash(state);
    }
}

impl Ord for Command {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.builtin.cmp(&other.builtin)
    }
}

impl PartialOrd for Command {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Borrow<Builtin> for Command {
    fn borrow(&self) -> &Builtin {
        &self.builtin
    }
}

impl Borrow<str> for Command {
    fn borrow(&self) -> &str {
        self.builtin.0.borrow()
    }
}

impl AsRef<str> for Command {
    fn as_ref(&self) -> &str {
        self.builtin.0.as_ref()
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.builtin)
    }
}

// TODO: replace with callable trait implementation if it's ever stabilized
// https://github.com/rust-lang/rust/issues/29625
impl Deref for Command {
    type Target = scallop::builtins::Builtin;

    fn deref(&self) -> &Self::Target {
        &self.builtin.0
    }
}

impl Command {
    ///  Explicitly set if a command calls `die` on failure.
    pub(crate) fn die(mut self, value: bool) -> Self {
        self.die_on_failure = value;
        self
    }

    /// Determine if the command is allowed in a given `Scope`.
    pub fn is_allowed<T>(&self, value: &T) -> bool
    where
        ScopeSet: PartialEq<T>,
    {
        self.allowed.iter().any(|x| x == value)
    }

    /// Determine if the command is a phase stub.
    pub fn is_phase(&self) -> bool {
        PhaseKind::from_str(self.as_ref()).is_ok()
    }
}

/// Try to parse the arguments for a given command.
trait TryParseArgs: Sized {
    fn try_parse_args(args: &[&str]) -> scallop::Result<Self>;
}

impl<P: clap::Parser> TryParseArgs for P {
    fn try_parse_args(args: &[&str]) -> scallop::Result<Self> {
        let cmd = Self::command();
        let name = cmd.get_name();

        // HACK: work around clap parsing always treating -- as a delimiter
        // See https://github.com/clap-rs/clap/issues/5055.
        let args = RawArgsIter {
            args: args.iter(),
            injected_arg: None,
            seen: false,
        };

        let args = [name].into_iter().chain(args);
        Self::try_parse_from(args).map_err(|e| scallop::Error::Base(format!("{name}: {e}")))
    }
}

struct RawArgsIter<'a> {
    args: std::slice::Iter<'a, &'a str>,
    injected_arg: Option<&'a str>,
    seen: bool,
}

impl<'a> Iterator for RawArgsIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.injected_arg.take().or_else(|| {
            self.args.next().copied().inspect(|x| {
                if *x == "--" && !self.seen {
                    let _ = self.injected_arg.insert(x);
                    self.seen = true;
                }
            })
        })
    }
}

/// Ordered set of all known builtins.
pub(crate) static BUILTINS: LazyLock<IndexSet<Builtin>> = LazyLock::new(|| {
    [
        builtins::adddeny,
        builtins::addpredict,
        builtins::addread,
        builtins::addwrite,
        builtins::assert,
        builtins::best_version,
        builtins::command_not_found_handle,
        builtins::debug_print,
        builtins::debug_print_function,
        builtins::debug_print_section,
        builtins::default,
        builtins::default_pkg_nofetch,
        builtins::default_src_compile,
        builtins::default_src_configure,
        builtins::default_src_install,
        builtins::default_src_prepare,
        builtins::default_src_test,
        builtins::default_src_unpack,
        builtins::die,
        builtins::diropts,
        builtins::dobin,
        builtins::docinto,
        builtins::docompress,
        builtins::doconfd,
        builtins::dodir,
        builtins::dodoc,
        builtins::doenvd,
        builtins::doexe,
        builtins::doheader,
        builtins::dohtml,
        builtins::doinfo,
        builtins::doinitd,
        builtins::doins,
        builtins::dolib,
        builtins::dolib_a,
        builtins::dolib_so,
        builtins::doman,
        builtins::domo,
        builtins::dosbin,
        builtins::dostrip,
        builtins::dosym,
        builtins::eapply,
        builtins::eapply_user,
        builtins::ebegin,
        builtins::econf,
        builtins::edo,
        builtins::eend,
        builtins::eerror,
        builtins::einfo,
        builtins::einfon,
        builtins::einstall,
        builtins::einstalldocs,
        builtins::elog,
        builtins::emake,
        builtins::eqawarn,
        builtins::ewarn,
        builtins::exeinto,
        builtins::exeopts,
        builtins::export_functions,
        builtins::fowners,
        builtins::fperms,
        builtins::get_libdir,
        builtins::has,
        builtins::has_version,
        builtins::hasq,
        builtins::hasv,
        builtins::in_iuse,
        builtins::inherit,
        builtins::insinto,
        builtins::insopts,
        builtins::into,
        builtins::keepdir,
        builtins::libopts,
        builtins::newbin,
        builtins::newconfd,
        builtins::newdoc,
        builtins::newenvd,
        builtins::newexe,
        builtins::newheader,
        builtins::newinitd,
        builtins::newins,
        builtins::newlib_a,
        builtins::newlib_so,
        builtins::newman,
        builtins::newsbin,
        builtins::nonfatal,
        builtins::pipestatus,
        builtins::unpack,
        builtins::use_,
        builtins::use_enable,
        builtins::use_with,
        builtins::useq,
        builtins::usev,
        builtins::usex,
        builtins::ver_cut,
        builtins::ver_replacing,
        builtins::ver_rs,
        builtins::ver_test,
        // phase stubs
        builtins::pkg_config,
        builtins::pkg_info,
        builtins::pkg_nofetch,
        builtins::pkg_postinst,
        builtins::pkg_postrm,
        builtins::pkg_preinst,
        builtins::pkg_prerm,
        builtins::pkg_pretend,
        builtins::pkg_setup,
        builtins::src_compile,
        builtins::src_configure,
        builtins::src_install,
        builtins::src_prepare,
        builtins::src_test,
        builtins::src_unpack,
    ]
    .into_iter()
    .collect()
});

/// USE flag variant for `use` and `usev` commands.
#[derive(Debug, Clone)]
struct UseFlag {
    flag: String,
    inverted: bool,
}

impl FromStr for UseFlag {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let (inverted, flag) = s.strip_prefix('!').map(|x| (true, x)).unwrap_or((false, s));

        crate::dep::parse::use_flag(flag).map(|value| Self {
            flag: value.to_string(),
            inverted,
        })
    }
}

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
            = start:$(['0'..='9']+) "-" end:$(['0'..='9']+) {?
                if let (Ok(start), Ok(end)) = (start.parse(), end.parse()) {
                    Ok((start, end))
                } else {
                    Err("range value overflow")
                }
            } / start:$(['0'..='9']+) "-" {?
                match start.parse() {
                    Ok(start) if start <= max => Ok((start, max)),
                    Ok(start) => Ok((start, start)),
                    _ => Err("range value overflow"),
                }
            } / start:$(['0'..='9']+) {?
                let start = start.parse().map_err(|_| "range value overflow")?;
                Ok((start, start))
            }
    }
}

// provide public parsing functionality while converting error types
mod parse {
    use crate::Error;
    use crate::error::peg_error;

    use super::cmd;

    pub(super) fn version_split(s: &str) -> crate::Result<Vec<&str>> {
        cmd::version_split(s).map_err(|e| peg_error("invalid version string", s, e))
    }

    pub(super) fn range(s: &str, max: usize) -> crate::Result<(usize, usize)> {
        let (start, end) = cmd::range(s, max).map_err(|e| peg_error("invalid range", s, e))?;
        if end < start {
            return Err(Error::InvalidValue(format!(
                "start of range ({start}) is greater than end ({end})",
            )));
        }
        Ok((start, end))
    }
}

/// Run a command given its name and argument list from bash.
fn run(name: &str, args: *mut scallop::bash::WordList) -> scallop::ExecStatus {
    use scallop::builtins::handle_error;
    use scallop::{Error, traits::IntoWords};

    let build = get_build_mut();
    let eapi = build.eapi();
    let scope = &build.scope;

    // run if enabled for the current build state
    let result = match eapi.commands().get(name) {
        Some(cmd) if cmd.is_allowed(scope) => {
            // convert raw command args into &str
            let args = args.to_words();
            let args: Result<Vec<_>, _> = args.into_iter().collect();
            // run command if args are valid utf8
            match args {
                Ok(args) => cmd.call(&args),
                Err(e) => Err(Error::Base(format!("invalid args: {e}"))),
            }
        }
        Some(_) => Err(Error::Base(format!("disabled in {scope} scope"))),
        None => Err(Error::Base(format!("disabled in EAPI {eapi}"))),
    };

    // handle errors, bailing out when running normally
    result.unwrap_or_else(|e| match e {
        Error::Base(s) if !build.nonfatal => handle_error(name, Error::Bail(s)),
        _ => handle_error(name, e),
    })
}

/// Create a static [`Builtin`] object for registry in bash.
#[macro_export]
macro_rules! make_builtin {
    ($name:expr, $func_name:ident) => {
        make_builtin!($name, $func_name, BUILTIN, "");
    };
    ($name:expr, $func_name:ident, $builtin:ident) => {
        make_builtin!($name, $func_name, $builtin, "");
    };
    ($name:expr, $func_name:ident, $builtin:ident, $usage:expr) => {
        #[unsafe(no_mangle)]
        extern "C" fn $func_name(args: *mut scallop::bash::WordList) -> std::ffi::c_int {
            i32::from($crate::shell::commands::run($name, args))
        }

        pub(crate) static $builtin: $crate::shell::commands::Builtin =
            $crate::shell::commands::Builtin(scallop::builtins::Builtin {
                name: $name,
                func: run,
                cfunc: $func_name,
                flags: scallop::builtins::Attr::ENABLED,
                help: "",
                usage: $usage,
            });
    };
}
use make_builtin;

#[cfg(test)]
fn assert_invalid_args(cmd: scallop::builtins::BuiltinFn, nums: &[u32]) {
    for n in nums {
        let args: Vec<_> = (0..*n).map(|n| n.to_string()).collect();
        let args: Vec<_> = args.iter().map(|s| s.as_str()).collect();
        let re = format!("^.*, got {n}");
        crate::test::assert_err_re!(cmd(&args), re);
    }
}

#[cfg(test)]
fn assert_invalid_cmd(cmd: scallop::builtins::BuiltinFn, nums: &[u32]) {
    for n in nums {
        let args: Vec<_> = (0..*n).map(|n| n.to_string()).collect();
        let args: Vec<_> = args.iter().map(|s| s.as_str()).collect();
        assert!(cmd(&args).is_err());
    }
}

#[cfg(test)]
macro_rules! cmd_scope_tests {
    ($cmd:expr) => {
        #[test]
        fn cmd_scope() {
            use std::collections::HashSet;

            use itertools::Itertools;

            use crate::config::Config;
            use crate::eapi::EAPIS_OFFICIAL;
            use crate::pkg::Source;
            use crate::repo::ebuild::EbuildRepoBuilder;
            use crate::shell::scope::{Scope, ScopeSet};
            use crate::test::assert_err_re;

            let cmd = $cmd;
            let mut args = cmd.split(' ').peekable();
            let name = args.next().unwrap();
            let has_args = args.peek().is_some();
            let invalid_cmd = [name]
                .into_iter()
                .chain(args.map(|_| r#"$'\x02\xc5\xd8'"#))
                .join(" ");
            let non_utf8_args_err = format!("{name}: error: invalid args: invalid utf-8 .*");
            let mut config = Config::default();
            let mut temp = EbuildRepoBuilder::new().build().unwrap();
            // create eclasses
            let eclass = indoc::formatdoc! {r#"
                # stub eclass
                VAR=1
                {cmd}
                VAR=2
            "#};
            temp.create_eclass("e1", &eclass).unwrap();
            let eclass = indoc::formatdoc! {r#"
                # stub eclass
                {invalid_cmd}
            "#};
            temp.create_eclass("invalid", &eclass).unwrap();
            let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
            config.finalize().unwrap();
            let all_scopes: HashSet<_> = ScopeSet::All.into_iter().collect();

            for eapi in &*EAPIS_OFFICIAL {
                if let Some(cmd) = eapi.commands().get(name) {
                    let scopes: HashSet<_> =
                        cmd.allowed.iter().flat_map(|x| x.iter()).collect();
                    // test non-utf8 args for commands that accept arguments
                    if has_args {
                        for scope in &scopes {
                            let info = format!("EAPI={eapi}, scope: {scope}");
                            match scope {
                                Scope::Eclass(_) => {
                                    let data = indoc::formatdoc! {r#"
                                        EAPI={eapi}
                                        inherit invalid
                                        DESCRIPTION="testing eclass scope invalid args"
                                        SLOT=0
                                    "#};
                                    temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
                                    let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
                                    let r = raw_pkg.source();
                                    assert_err_re!(r, non_utf8_args_err, &info);
                                }
                                Scope::Global => {
                                    let data = indoc::formatdoc! {r#"
                                        EAPI={eapi}
                                        DESCRIPTION="testing global scope failures"
                                        SLOT=0
                                        VAR=1
                                        {invalid_cmd}
                                        VAR=2
                                    "#};
                                    temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
                                    let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
                                    let r = raw_pkg.source();
                                    assert_err_re!(r, non_utf8_args_err, &info);
                                }
                                Scope::Phase(phase) => {
                                    let data = indoc::formatdoc! {r#"
                                        EAPI={eapi}
                                        DESCRIPTION="testing phase scope failures"
                                        SLOT=0
                                        VAR=1
                                        {phase}() {{
                                            {invalid_cmd}
                                            VAR=2
                                        }}
                                    "#};
                                    temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
                                    let pkg = repo.get_pkg("cat/pkg-1").unwrap();
                                    pkg.source().unwrap();
                                    let phase = eapi.phases().get(phase).unwrap();
                                    let r = phase.run();
                                    assert_err_re!(r, non_utf8_args_err, &info);
                                }
                            }
                        }
                    }

                    // test invalid scope usage
                    for scope in all_scopes.difference(&scopes) {
                        let info = format!("EAPI={eapi}, scope: {scope}");
                        match scope {
                            Scope::Eclass(_) => {
                                let data = indoc::formatdoc! {r#"
                                    EAPI={eapi}
                                    inherit e1
                                    DESCRIPTION="testing eclass scope failures"
                                    SLOT=0
                                "#};
                                temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
                                let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
                                let r = raw_pkg.source();
                                // verify sourcing stops at unknown command
                                assert_eq!(scallop::variables::optional("VAR").unwrap(), "1");
                                // verify error output
                                let err = format!("{name}: error: disabled in eclass scope");
                                assert_err_re!(r, err, &info);
                            }
                            Scope::Global => {
                                let data = indoc::formatdoc! {r#"
                                    EAPI={eapi}
                                    DESCRIPTION="testing global scope failures"
                                    SLOT=0
                                    VAR=1
                                    {cmd}
                                    VAR=2
                                "#};
                                temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
                                let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
                                let r = raw_pkg.source();
                                // verify sourcing stops at unknown command
                                assert_eq!(scallop::variables::optional("VAR").unwrap(), "1");
                                // verify error output
                                let err = format!("{name}: error: disabled in global scope");
                                assert_err_re!(r, err, &info);
                            }
                            Scope::Phase(phase) => {
                                let data = indoc::formatdoc! {r#"
                                    EAPI={eapi}
                                    DESCRIPTION="testing phase scope failures"
                                    SLOT=0
                                    VAR=1
                                    {phase}() {{
                                        {cmd}
                                        VAR=2
                                    }}
                                "#};
                                temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
                                let pkg = repo.get_pkg("cat/pkg-1").unwrap();
                                pkg.source().unwrap();
                                let phase = eapi.phases().get(phase).unwrap();
                                let r = phase.run();
                                // verify function stops at unknown command
                                assert_eq!(
                                    scallop::variables::optional("VAR").as_deref(),
                                    Some("1")
                                );
                                // verify error output
                                let err = format!("{name}: error: disabled in {phase} scope");
                                assert_err_re!(r, err, &info);
                            }
                        }
                    }
                } else {
                    let data = indoc::formatdoc! {r#"
                        EAPI={eapi}
                        DESCRIPTION="testing command disabled in EAPI failures"
                        SLOT=0
                        VAR=1
                        {cmd}
                        VAR=2
                    "#};
                    temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
                    let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
                    let r = raw_pkg.source();
                    // verify sourcing stops at unknown command
                    assert_eq!(scallop::variables::optional("VAR").unwrap(), "1");
                    // verify error output
                    let err = format!("{name}: error: disabled in EAPI {eapi}");
                    assert_err_re!(r, err);
                }
            }
        }
    };
}
#[cfg(test)]
use cmd_scope_tests;

#[cfg(test)]
mod tests {
    use crate::eapi::EAPI_LATEST_OFFICIAL;

    #[test]
    fn command_traits() {
        let dobin = EAPI_LATEST_OFFICIAL.commands().get("dobin").unwrap();
        assert_eq!(dobin.to_string(), "dobin");
        assert!(format!("{dobin:?}").contains("dobin"));

        let dodir = EAPI_LATEST_OFFICIAL.commands().get("dodir").unwrap();
        assert!(dobin != dodir);
        assert!(dobin < dodir);
    }
}
