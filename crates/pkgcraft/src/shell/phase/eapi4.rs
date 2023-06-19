use scallop::builtins::ExecStatus;

use crate::shell::builtins::einstalldocs::install_docs_from;
use crate::shell::BuildData;

use super::emake_install;

pub(crate) fn src_install(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    emake_install(build)?;
    install_docs_from("DOCS")
}
