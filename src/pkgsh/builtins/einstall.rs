use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::{emake::run as emake, PkgBuiltin};
use crate::pkgsh::utils::get_libdir;
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Run `emake install` for a package.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let env = &d.borrow().env;
        #[allow(non_snake_case)]
        let ED = env
            .get("ED")
            .unwrap_or_else(|| env.get("D").expect("$D undefined"));
        let paths: &[&str] = &[
            &format!("prefix={}/usr", ED),
            &format!("datadir={}/usr/share", ED),
            &format!("mandir={}/usr/share/man", ED),
            &format!("infodir={}/usr/share/info", ED),
            // Note that the additional complexity for determining libdir described in PMS is
            // ignored in favor of using the more modern and simple value from get_libdir().
            &format!("libdir={}/usr/{}", ED, get_libdir()),
            &format!("localstatedir={}/var/lib", ED),
            &format!("sysconfdir={}/etc", ED),
        ];

        let args = &[paths, &["-j1"], args, &["install"]].concat();
        emake(args)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "einstall",
            func: run,
            help: LONG_DOC,
            usage: "einstall",
        },
        &[("0-5", &["src_install"])],
    )
});
