use scallop::builtins::ExecStatus;
use scallop::Result;

use super::emake_install;
use crate::pkgsh::builtins::einstalldocs::run as einstalldocs;

pub(crate) fn src_install() -> Result<ExecStatus> {
    emake_install()?;
    einstalldocs(&[])
}
