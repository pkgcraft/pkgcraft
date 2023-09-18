use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::shell::write_stdout;

use super::use_::run as use_;

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
        (ExecStatus::Success, None) => write_stdout!("--{enabled}-{opt}")?,
        (ExecStatus::Success, Some(value)) => write_stdout!("--{enabled}-{opt}={value}")?,
        (ExecStatus::Failure(_), None) => write_stdout!("--{disabled}-{opt}")?,
        (ExecStatus::Failure(_), Some(value)) => write_stdout!("--{disabled}-{opt}={value}")?,
    }

    Ok(ret)
}
