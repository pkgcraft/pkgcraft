use scallop::builtins::ExecStatus;
use scallop::Result;

use crate::pkgsh::utils::get_libdir;
use crate::pkgsh::BUILD_DATA;

use super::{emake::run as emake, make_builtin};

const LONG_DOC: &str = "Run `emake install` for a package.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();
        let destdir = d.destdir();
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
    })
}

const USAGE: &str = "einstall";
make_builtin!("einstall", einstall_builtin, run, LONG_DOC, USAGE, &[("0-5", &["src_install"])]);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);

    // TODO: add usage tests
}
