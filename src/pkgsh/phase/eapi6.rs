use scallop::builtins::ExecStatus;
use scallop::variables::var_to_vec;

use super::emake_install;
use crate::pkgsh::builtins::{
    eapply::run as eapply, eapply_user::run as eapply_user, einstalldocs::run as einstalldocs,
};

pub(crate) fn src_prepare() -> scallop::Result<ExecStatus> {
    if let Ok(patches) = var_to_vec("PATCHES") {
        if !patches.is_empty() {
            // Note that not allowing options in PATCHES is technically from EAPI 8, but it's
            // backported here for EAPI 6 onwards.
            let mut args = vec!["--"];
            // TODO: need to perform word expansion on each string as well
            args.extend(patches.iter().map(|s| s.as_str()));
            eapply(&args)?;
        }
    }
    eapply_user(&[])
}

pub(crate) fn src_install() -> scallop::Result<ExecStatus> {
    emake_install()?;
    einstalldocs(&[])
}
