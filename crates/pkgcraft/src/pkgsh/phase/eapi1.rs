use scallop::builtins::ExecStatus;

use super::eapi2;

pub(crate) fn src_compile() -> scallop::Result<ExecStatus> {
    eapi2::src_configure()?;
    eapi2::src_compile()
}
