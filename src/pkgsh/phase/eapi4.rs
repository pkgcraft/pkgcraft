use scallop::builtins::ExecStatus;

use crate::pkgsh::builtins::einstalldocs::install_docs_from;

use super::emake_install;

pub(crate) fn src_install() -> scallop::Result<ExecStatus> {
    emake_install()?;
    install_docs_from("DOCS")
}
