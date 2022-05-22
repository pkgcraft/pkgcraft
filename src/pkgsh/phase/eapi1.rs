use scallop::builtins::ExecStatus;
use scallop::Result;

use super::eapi2;

pub(crate) fn src_compile() -> Result<ExecStatus> {
    eapi2::src_configure()?;
    eapi2::src_compile()
}
