use std::io::stdout;
use std::process::Command;
use std::str;

use indexmap::{IndexMap, IndexSet};
use is_executable::IsExecutable;
use once_cell::sync::Lazy;
use regex::Regex;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::variables::{expand, string_value};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::utils::{configure, output_command};
use crate::pkgsh::BUILD_DATA;

static CONFIG_OPT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<opt>--[^=\s]+)(=(?P<val>\w+))?").unwrap());

static LONG_DOC: &str = "Run a package's configure script.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let configure = configure();
    if !configure.is_executable() {
        match configure.exists() {
            true => return Err(Error::Builtin("nonexecutable configure script".into())),
            false => return Err(Error::Builtin("nonexistent configure script".into())),
        }
    }

    let mut args_map = IndexMap::<&str, Option<String>>::new();
    for arg in args {
        match arg.split_once('=') {
            None => args_map.insert(arg, None),
            Some((opt, val)) => args_map.insert(opt, Some(val.to_string())),
        };
    }

    let conf_help = Command::new(&configure)
        .arg("--help")
        .output()
        .map_err(|e| Error::Builtin(format!("failed running: {}", e)))?;
    let mut known_opts = IndexSet::<String>::new();
    let conf_help = str::from_utf8(&conf_help.stdout).expect("failed decoding configure output");
    for line in conf_help.split('\n') {
        for caps in CONFIG_OPT_RE.captures_iter(line.trim()) {
            known_opts.insert(caps["opt"].to_string());
        }
    }

    let mut defaults = IndexMap::<&str, Option<String>>::new();
    BUILD_DATA.with(|d| {
        let env = &d.borrow().env;
        let eprefix = env.get("EPREFIX").expect("$EPREFIX undefined");
        let chost = env.get("CHOST").expect("$CHOST undefined");

        // TODO: add libdir setting
        for (opt, val) in [
            ("--prefix", format!("{}/usr", eprefix)),
            ("--mandir", format!("{}/usr/share/man", eprefix)),
            ("--infodir", format!("{}/usr/share/info", eprefix)),
            ("--datadir", format!("{}/usr/share", eprefix)),
            ("--sysconfdir", format!("{}/etc", eprefix)),
            ("--localstatedir", format!("{}/var/lib", eprefix)),
            ("--host", chost.clone()),
        ] {
            defaults.insert(opt, Some(val));
        }

        for (opt, var) in [("build", "CBUILD"), ("target", "CTARGET")] {
            if let Some(val) = string_value(var) {
                defaults.insert(opt, Some(val));
            }
        }

        // add EAPI specific options if found
        for (opt, (markers, val)) in d.borrow().eapi.econf_options() {
            if !known_opts.is_disjoint(markers) {
                match val {
                    None => defaults.insert(opt, None),
                    Some(v) => defaults.insert(opt, Some(expand(v).unwrap())),
                };
            }
        }
    });

    let mut econf = Command::new(&configure);
    defaults.extend(args_map.into_iter());
    for (opt, val) in defaults.iter() {
        match val {
            None => econf.arg(opt),
            Some(v) => econf.arg(format!("{}={}", opt, v)),
        };
    }

    output_command(stdout(), &econf);

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

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs::File;
    use std::io::prelude::*;
    use std::process::{Command, Stdio};

    use super::run as econf;
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    use indoc::indoc;
    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    static CONFIGURE_AC: &str = indoc! {"
        AC_INIT([pkgcraft], [0.0.1], [pkgcraft@pkgcraft.org])
        AM_INIT_AUTOMAKE([-Wall -Werror foreign])
        AC_PROG_CC
        AC_OUTPUT
    "};

    rusty_fork_test! {
        #[test]
        fn nonexistent() {
            assert_err_re!(econf(&[]), format!("^nonexistent configure .*$"));
        }

        #[test]
        fn nonexecutable() {
            let dir = tempdir().unwrap();
            let configure = dir.path().join("configure");
            File::create(configure).unwrap();
            env::set_current_dir(&dir).unwrap();
            assert_err_re!(econf(&[]), format!("^nonexecutable configure .*$"));
        }

        #[test]
        #[cfg_attr(target_os = "macos", ignore)]
        fn configure_parsing() {
            let dir = tempdir().unwrap();
            let configure_ac = dir.path().join("configure.ac");
            let file = File::create(configure_ac).unwrap();
            env::set_current_dir(&dir).unwrap();
            write!(&file, "{}", CONFIGURE_AC).unwrap();
            Command::new("autoreconf").arg("-i").stderr(Stdio::null()).status().unwrap();
            BUILD_DATA.with(|d| {
                // TODO: add support for generating build state data for tests
                d.borrow_mut().env.extend([
                    ("EPREFIX".into(), "/eprefix".into()),
                    ("CHOST".into(), "chost".into()),
                ]);
                econf(&[]).unwrap();
            });
        }
    }
}
