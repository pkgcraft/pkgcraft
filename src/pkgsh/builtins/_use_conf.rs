use std::io::Write;

use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use super::use_::run as use_;
use crate::pkgsh::{write_stdout, BUILD_DATA};

// Underlying implementation for use_with and use_enable.
pub(super) fn use_conf(args: &[&str], enabled: &str, disabled: &str) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (flag, opt, suffix) = match args.len() {
            1 => match args[0].starts_with('!') {
                false => (&args[..1], args[0], String::from("")),
                true => {
                    return Err(Error::Builtin("USE flag inversion requires 2 or 3 args".into()))
                }
            },
            2 => (&args[..1], args[1], String::from("")),
            3 => match eapi.has("use_conf_arg") {
                true => (&args[..1], args[1], format!("={}", args[2])),
                false => return Err(Error::Builtin("requires 1 or 2 args, got 3".into())),
            },
            n => return Err(Error::Builtin(format!("requires 1, 2, or 3 args, got {}", n))),
        };

        let ret = use_(flag)?;
        match ret {
            ExecStatus::Success => write_stdout!("--{}-{}{}", enabled, opt, suffix),
            ExecStatus::Failure => write_stdout!("--{}-{}{}", disabled, opt, suffix),
            _ => (),
        }
        Ok(ret)
    })
}
