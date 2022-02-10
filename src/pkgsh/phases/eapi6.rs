use scallop::builtins::ExecStatus;
use scallop::Result;

use crate::pkgsh::builtins::{einstalldocs::run as einstalldocs, emake::run as emake};
use crate::pkgsh::utils::makefile_exists;
use crate::pkgsh::BUILD_DATA;

pub(crate) fn src_install() -> Result<ExecStatus> {
    if makefile_exists() {
        BUILD_DATA.with(|d| -> Result<ExecStatus> {
            let env = &d.borrow().env;
            #[allow(non_snake_case)]
            let D = env.get("D").expect("D undefined");
            let destdir = format!("DESTDIR={}", D);
            let args = &[destdir.as_str(), "install"];
            emake(args)
        })?;
    }
    einstalldocs(&[])
}
