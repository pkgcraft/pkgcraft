use std::path::Path;

use is_executable::IsExecutable;
use scallop::builtins::ExecStatus;
use scallop::variables::expand;
use scallop::Result;

use super::super::builtins::econf::run as econf;

pub(crate) fn src_configure() -> Result<ExecStatus> {
    let path = expand("${ECONF_SOURCE:-.}/configure").unwrap();
    let configure = Path::new(&path);
    match configure.is_executable() {
        true => econf(&[]),
        false => Ok(ExecStatus::Success),
    }
}
