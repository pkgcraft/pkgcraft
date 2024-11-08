use scallop::ExecStatus;

use crate::shell::BuildData;

/// Set docompress include/exclude defaults for supported EAPIs.
pub(crate) fn pre(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    let docompress_include_defaults = ["/usr/share/doc", "/usr/share/info", "/usr/share/man"]
        .into_iter()
        .map(String::from);
    let docompress_exclude_defaults = [format!("/usr/share/doc/{}/html", build.cpv().pf())];
    build.compress_include.extend(docompress_include_defaults);
    build.compress_exclude.extend(docompress_exclude_defaults);
    Ok(ExecStatus::Success)
}

pub(crate) fn post(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    // TODO: perform docompress operation
    Ok(ExecStatus::Success)
}
