use scallop::{Error, ExecStatus};

use crate::shell::BuildData;

pub(crate) fn post(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    if !build.user_patches_applied {
        Err(Error::Base("eapply_user was not called during src_prepare()".to_string()))
    } else {
        Ok(ExecStatus::Success)
    }
}
