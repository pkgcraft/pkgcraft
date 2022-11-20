use scallop::builtins::ExecStatus;

use super::emake_install;
use crate::pkgsh::builtins::einstalldocs::install_docs_from;

pub(crate) fn src_install() -> scallop::Result<ExecStatus> {
    emake_install()?;
    install_docs_from("DOCS")
}
