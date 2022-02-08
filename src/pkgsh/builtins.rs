use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

use indexmap::IndexSet;
use once_cell::sync::Lazy;
use regex::Regex;
use scallop::builtins::{Builtin, ExecStatus};

use crate::{eapi, eapi::Eapi};

mod _use_conf;
pub mod assert;
pub mod debug_print;
pub mod debug_print_function;
pub mod debug_print_section;
pub mod default;
pub mod die;
pub mod diropts;
pub mod docinto;
pub mod docompress;
pub mod dodoc;
pub mod dostrip;
pub mod einstalldocs;
pub mod exeinto;
pub mod exeopts;
pub mod export_functions;
pub mod get_libdir;
pub mod has;
pub mod hasq;
pub mod hasv;
pub mod in_iuse;
pub mod inherit;
pub mod insinto;
pub mod insopts;
pub mod into;
pub mod libopts;
pub mod nonfatal;
pub mod use_;
pub mod use_enable;
pub mod use_with;
pub mod useq;
pub mod usev;
pub mod usex;
pub mod ver_cut;
pub mod ver_rs;
pub mod ver_test;

pub(crate) struct PkgBuiltin {
    builtin: Builtin,
    eapis: IndexSet<&'static Eapi>,
    scope_re: Regex,
}

// scope patterns
static ECLASS: &str = "eclass";
static GLOBAL: &str = ".+";
static PHASE: &str = ".+_.+";

impl PkgBuiltin {
    fn new(builtin: Builtin, eapis: &str, scope: &[&str]) -> Self {
        PkgBuiltin {
            builtin,
            eapis: eapi::supported(eapis).expect("failed to parse EAPI range"),
            scope_re: Regex::new(&format!(r"^{}$", scope.join("|"))).unwrap(),
        }
    }

    #[inline]
    pub(crate) fn run(&self, args: &[&str]) -> scallop::Result<ExecStatus> {
        self.builtin.run(args)
    }
}

pub(crate) type BuiltinsMap = HashMap<String, &'static PkgBuiltin>;
pub(crate) type PhaseBuiltinsMap = HashMap<String, BuiltinsMap>;
pub(crate) type EapiBuiltinsMap = HashMap<&'static Eapi, PhaseBuiltinsMap>;

pub(crate) static BUILTINS_MAP: Lazy<EapiBuiltinsMap> = Lazy::new(|| {
    let builtins: Vec<&PkgBuiltin> = vec![
        &assert::BUILTIN,
        &debug_print::BUILTIN,
        &debug_print_function::BUILTIN,
        &debug_print_section::BUILTIN,
        &default::BUILTIN,
        &die::BUILTIN,
        &diropts::BUILTIN,
        &docinto::BUILTIN,
        &docompress::BUILTIN,
        &dodoc::BUILTIN,
        &dostrip::BUILTIN,
        &einstalldocs::BUILTIN,
        &exeinto::BUILTIN,
        &exeopts::BUILTIN,
        &export_functions::BUILTIN,
        &get_libdir::BUILTIN,
        &has::BUILTIN,
        &hasq::BUILTIN,
        &hasv::BUILTIN,
        &in_iuse::BUILTIN,
        &inherit::BUILTIN,
        &insinto::BUILTIN,
        &into::BUILTIN,
        &libopts::BUILTIN,
        &nonfatal::BUILTIN,
        &use_::BUILTIN,
        &use_enable::BUILTIN,
        &use_with::BUILTIN,
        &useq::BUILTIN,
        &usev::BUILTIN,
        &usex::BUILTIN,
        &ver_cut::BUILTIN,
        &ver_rs::BUILTIN,
        &ver_test::BUILTIN,
    ];
    let mut builtins_map = EapiBuiltinsMap::new();

    for b in builtins.iter() {
        for eapi in b.eapis.iter() {
            let phase_map = builtins_map
                .entry(eapi)
                .or_insert_with(PhaseBuiltinsMap::new);
            for phase in eapi.phases().iter() {
                if b.scope_re.is_match(phase) {
                    phase_map
                        .entry(phase.clone())
                        .or_insert_with(BuiltinsMap::new)
                        .insert(b.builtin.name.to_string(), b);
                }
            }

            if b.scope_re.is_match("global") {
                phase_map
                    .entry("global".to_string())
                    .or_insert_with(BuiltinsMap::new)
                    .insert(b.builtin.name.to_string(), b);
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

    #[inline]
    pub(crate) fn range(s: &str, max: usize) -> Result<(usize, usize)> {
        let (start, end) =
            cmd::range(s, max).map_err(|e| peg_error(format!("invalid range: {:?}", s), s, e))?;
        if end < start {
            return Err(Error::InvalidValue(format!(
                "start of range ({}) is greater than end ({})",
                start, end
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
        let re = format!("^.*, got {}", n);
        crate::macros::assert_err_re!(func(args.as_slice()), re);
    }
}
