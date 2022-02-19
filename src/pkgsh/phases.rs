use scallop::builtins::ExecStatus;
use scallop::Result;

use super::builtins::emake::run as emake;
use super::utils::makefile_exists;
use super::BUILD_DATA;

pub(crate) mod eapi0;
pub(crate) mod eapi1;
pub(crate) mod eapi2;
pub(crate) mod eapi4;
pub(crate) mod eapi6;

pub(crate) type PhaseFn = fn() -> Result<ExecStatus>;

pub(crate) fn phase_stub() -> Result<ExecStatus> {
    Ok(ExecStatus::Success)
}

pub(super) fn emake_install() -> Result<ExecStatus> {
    if makefile_exists() {
        BUILD_DATA.with(|d| -> Result<ExecStatus> {
            let env = &d.borrow().env;
            #[allow(non_snake_case)]
            let D = env.get("D").expect("D undefined");
            let destdir = format!("DESTDIR={D}");
            let args = &[destdir.as_str(), "install"];
            emake(args)
        })?;
    }

    Ok(ExecStatus::Success)
}
