use scallop::builtins::ExecStatus;
use scallop::Result;

use super::emake_install;
use crate::pkgsh::builtins::einstalldocs::install_docs;

pub(crate) fn src_install() -> Result<ExecStatus> {
    emake_install()?;
    install_docs("DOCS")
}
