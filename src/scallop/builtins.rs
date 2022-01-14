use std::sync::atomic::AtomicBool;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::scallop::BUILD_DATA;

pub mod assert;
pub mod debug_print;
pub mod debug_print_function;
pub mod debug_print_section;
pub mod die;
pub mod export_functions;
pub mod has;
pub mod hasv;
pub mod in_iuse;
pub mod inherit;
pub mod nonfatal;
pub mod r#use;
pub mod use_enable;
pub mod use_with;
pub mod usex;
pub mod ver_cut;
pub mod ver_rs;
pub mod ver_test;

static NONFATAL: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

static VERSION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?P<sep>[[:^alnum:]]+)?(?P<comp>[[:digit:]]+|[[:alpha:]]+)?").unwrap()
});

/// Split version string into a vector of separators and components.
pub(crate) fn version_split(ver: &str) -> Vec<&str> {
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
    pub grammar cmd() for str {
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

pub(crate) fn use_conf(args: &[&str], enabled: &str, disabled: &str) -> scallop::Result<i32> {
    BUILD_DATA.with(|d| -> scallop::Result<i32> {
        let eapi = d.borrow().eapi;
        let (flag, opt, suffix) = match args.len() {
            1 => (&args[..1], args[0], String::from("")),
            2 => (&args[..1], args[1], String::from("")),
            3 => match eapi.has("use_conf_arg") {
                true => (&args[..1], args[1], format!("={}", args[2])),
                false => return Err(scallop::Error::new("requires 1 or 2 args, got 3")),
            },
            n => {
                return Err(scallop::Error::new(format!(
                    "requires 1, 2, or 3 args, got {}",
                    n
                )))
            }
        };

        let ret = r#use::run(flag)?;
        match ret {
            0 => println!("--{}-{}{}", enabled, opt, suffix),
            1 => println!("--{}-{}{}", disabled, opt, suffix),
            n => panic!("invalid return value: {}", n),
        }
        Ok(ret)
    })
}
