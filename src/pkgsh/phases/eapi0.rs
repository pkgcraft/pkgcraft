use scallop::Result;

use super::super::{builtins::unpack, BUILD_DATA};

pub(crate) fn src_unpack() -> Result<()> {
    BUILD_DATA.with(|d| -> Result<()> {
        let distfiles = &d.borrow().distfiles;
        let args: Vec<&str> = distfiles.iter().map(|s| s.as_str()).collect();
        unpack::run(&args).map(|_| ())
    })
}
