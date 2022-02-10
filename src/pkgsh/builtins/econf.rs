use std::process::Command;

use is_executable::IsExecutable;
use itertools::join;
use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::variables::{expand, string_value};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::utils::configure;
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Run a package's configure script.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let configure = configure();
    if !configure.is_executable() {
        if !configure.exists() {
            return Err(Error::Builtin("nonexecutable configure script".into()));
        }
        return Err(Error::Builtin("nonexistent configure script".into()));
    }

    let conf_help = Command::new(&configure)
        .arg("--help")
        .output()
        .map_err(|e| Error::Builtin(format!("failed running: {}", e)))?;
    // TODO: match against raw bytes from stdout instead of joining to a string?
    let conf_help = join(conf_help.stdout, "");

    let mut econf = Command::new(&configure);

    BUILD_DATA.with(|d| {
        let env = &d.borrow().env;
        let chost = env.get("CHOST").expect("$CHOST undefined");
        let eprefix = env.get("EPREFIX").expect("$EPREFIX undefined");

        // TODO: add libdir setting
        econf.args([
            format!("--prefix={}/usr", eprefix),
            format!("--mandir={}/usr/share/man", eprefix),
            format!("--infodir={}/usr/share/info", eprefix),
            format!("--datadir={}/usr/share", eprefix),
            format!("--sysconfdir={}/etc", eprefix),
            format!("--localstatedir={}/etc", eprefix),
            format!("--host={}", chost),
        ]);

        for (opt, var) in [("build", "CBUILD"), ("target", "CTARGET")] {
            if let Some(val) = string_value(var) {
                econf.arg(format!("--{}={}", opt, val));
            }
        }

        // add EAPI specific options if found
        for (opt, (re, val)) in d.borrow().eapi.econf_options() {
            if re.is_match(&conf_help) {
                let option = match val {
                    Some(x) => format!("--{}={}", opt, expand(x).unwrap()),
                    None => format!("--{}", opt),
                };
                econf.arg(option);
            }
        }
    });

    econf.args(args);
    econf.status().map_or_else(
        |e| Err(Error::Builtin(format!("failed running: {}", e))),
        |v| Ok(ExecStatus::from(v)),
    )
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "econf",
            func: run,
            help: LONG_DOC,
            usage: "econf --enable-feature",
        },
        &[("0-1", &["src_compile"]), ("2-", &["src_configure"])],
    )
});
