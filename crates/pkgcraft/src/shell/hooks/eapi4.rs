use once_cell::sync::Lazy;
use scallop::builtins::ExecStatus;

use crate::shell::phase::PhaseKind;
use crate::shell::BuildData;

use super::{Hook, HookKind};

pub(crate) static HOOKS: Lazy<Vec<(PhaseKind, HookKind, Vec<Hook>)>> = Lazy::new(|| {
    [
        (
            PhaseKind::SrcInstall,
            HookKind::Pre,
            vec![Hook::new("docompress", docompress_pre, 0, false)],
        ),
        (
            PhaseKind::SrcInstall,
            HookKind::Post,
            vec![Hook::new("docompress", docompress_post, 0, false)],
        ),
    ]
    .into_iter()
    .collect()
});

/// Set docompress include/exclude defaults for supported EAPIs.
fn docompress_pre(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    let docompress_include_defaults = ["/usr/share/doc", "/usr/share/info", "/usr/share/man"]
        .into_iter()
        .map(String::from);
    let docompress_exclude_defaults = [format!("/usr/share/doc/{}/html", build.cpv()?.pf())];
    build.compress_include.extend(docompress_include_defaults);
    build.compress_exclude.extend(docompress_exclude_defaults);
    Ok(ExecStatus::Success)
}

fn docompress_post(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    // TODO: perform docompress operation
    Ok(ExecStatus::Success)
}
