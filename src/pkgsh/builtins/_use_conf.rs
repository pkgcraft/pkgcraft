use std::io::{stdout, Write};

use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use super::use_;
use crate::macros::write_flush;
use crate::pkgsh::BUILD_DATA;

// Underlying implementation for use_with and use_enable.
pub(crate) fn use_conf(args: &[&str], enabled: &str, disabled: &str) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (flag, opt, suffix) = match args.len() {
            1 => (&args[..1], args[0], String::from("")),
            2 => (&args[..1], args[1], String::from("")),
            3 => match eapi.has("use_conf_arg") {
                true => (&args[..1], args[1], format!("={}", args[2])),
                false => return Err(Error::new("requires 1 or 2 args, got 3")),
            },
            n => return Err(Error::new(format!("requires 1, 2, or 3 args, got {}", n))),
        };

        let ret = use_::run(flag)?;
        match ret {
            ExecStatus::Success => write_flush!(stdout(), "--{}-{}{}", enabled, opt, suffix),
            ExecStatus::Failure => write_flush!(stdout(), "--{}-{}{}", disabled, opt, suffix),
        }
        Ok(ret)
    })
}
