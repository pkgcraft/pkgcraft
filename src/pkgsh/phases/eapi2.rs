use std::path::PathBuf;

use is_executable::IsExecutable;
use scallop::builtins::ExecStatus;
use scallop::variables::string_value;
use scallop::Result;

use super::super::builtins::econf;

pub(crate) fn src_configure() -> Result<ExecStatus> {
    let dir = string_value("ECONF_SOURCE").unwrap_or_else(|| String::from("."));
    let configure: PathBuf = [&dir, "configure"].iter().collect();
    match configure.is_executable() {
        true => econf::run(&[]),
        false => Ok(ExecStatus::Success),
    }
}
