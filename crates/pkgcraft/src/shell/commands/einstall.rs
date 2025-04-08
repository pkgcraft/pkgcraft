use scallop::ExecStatus;

use crate::shell::get_build_mut;
use crate::shell::utils::get_libdir;

use super::{emake, make_builtin};

const LONG_DOC: &str = "Run `emake install` for a package.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let destdir = get_build_mut().destdir();
    let paths: &[&str] = &[
        &format!("prefix={destdir}/usr"),
        &format!("datadir={destdir}/usr/share"),
        &format!("mandir={destdir}/usr/share/man"),
        &format!("infodir={destdir}/usr/share/info"),
        // Note that the additional complexity for determining libdir described in PMS is
        // ignored in favor of using the more modern and simple value from get_libdir().
        &format!("libdir={destdir}/usr/{}", get_libdir(Some("lib")).unwrap()),
        &format!("localstatedir={destdir}/var/lib"),
        &format!("sysconfdir={destdir}/etc"),
    ];

    emake(&[paths, &["-j1"], args, &["install"]].concat())
}

make_builtin!("einstall", einstall_builtin, true);

#[cfg(test)]
mod tests {
    use super::super::cmd_scope_tests;

    cmd_scope_tests!("einstall");

    // TODO: add usage tests
}
