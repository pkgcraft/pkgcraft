use scallop::builtins::ExecStatus;

use crate::pkgsh::BuildData;

use super::eapi2;

pub(crate) fn src_compile(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    eapi2::src_configure(build)?;
    eapi2::src_compile(build)
}
