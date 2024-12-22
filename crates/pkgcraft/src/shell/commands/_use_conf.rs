use std::io::Write;

use scallop::{Error, ExecStatus};

use crate::io::stdout;

use super::use_;

// Underlying implementation for use_with and use_enable.
pub(super) fn use_conf(
    args: &[&str],
    enabled: &str,
    disabled: &str,
) -> scallop::Result<ExecStatus> {
    let (flag, opt, value) = match args[..] {
        [flag] if flag.starts_with('!') => {
            return Err(Error::Base("USE flag inversion requires 2 or 3 args".into()))
        }
        [flag] => (flag, flag, None),
        [flag, opt] => (flag, opt, None),
        [flag, opt, value] => (flag, opt, Some(value)),
        _ => return Err(Error::Base(format!("requires 1, 2, or 3 args, got {}", args.len()))),
    };

    let ret = use_(&[flag])?;

    match (ret, value) {
        (ExecStatus::Success, None) => write!(stdout(), "--{enabled}-{opt}")?,
        (ExecStatus::Success, Some(value)) => write!(stdout(), "--{enabled}-{opt}={value}")?,
        (ExecStatus::Failure(_), None) => write!(stdout(), "--{disabled}-{opt}")?,
        (ExecStatus::Failure(_), Some(value)) => {
            write!(stdout(), "--{disabled}-{opt}={value}")?
        }
    }

    Ok(ret)
}
