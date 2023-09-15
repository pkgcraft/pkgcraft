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
    let (flag, opt, suffix) = match args.len() {
        1 => {
            if args[0].starts_with('!') {
                Err(Error::Base("USE flag inversion requires 2 or 3 args".into()))
            } else {
                Ok((&args[..1], args[0], String::from("")))
            }
        }
        2 => Ok((&args[..1], args[1], String::from(""))),
        3 => Ok((&args[..1], args[1], format!("={}", args[2]))),
        n => Err(Error::Base(format!("requires 1, 2, or 3 args, got {n}"))),
    }?;

    let ret = use_(flag)?;
    match ret {
        ExecStatus::Success => write_stdout!("--{enabled}-{opt}{suffix}")?,
        ExecStatus::Failure(_) => write_stdout!("--{disabled}-{opt}{suffix}")?,
    }
    Ok(ret)
}
