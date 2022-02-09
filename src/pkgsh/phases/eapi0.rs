use scallop::builtins::ExecStatus;
use scallop::Result;

use super::super::{builtins::unpack, BUILD_DATA};

pub(crate) fn src_unpack() -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let distfiles = &d.borrow().distfiles;
        let args: Vec<&str> = distfiles.iter().map(|s| s.as_str()).collect();
        match !args.is_empty() {
            true => unpack::run(&args),
            false => Ok(ExecStatus::Success),
        }
    })
}
