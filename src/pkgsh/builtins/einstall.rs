use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::{emake::run as emake, PkgBuiltin};
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Run `emake install` for a package.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let env = &d.borrow().env;
        #[allow(non_snake_case)]
        let ED = env.get("ED").expect("ED undefined");
        let paths: &[&str] = &[
            &format!("prefix={}/usr", ED),
            &format!("datadir={}/usr/share", ED),
            &format!("infodir={}/usr/share/info", ED),
            &format!("localstatedir={}/var/lib", ED),
            &format!("mandir={}/usr/share/man", ED),
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
