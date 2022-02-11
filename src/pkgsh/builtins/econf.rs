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
use crate::pkgsh::utils::{configure, get_libdir, output_command};
use crate::pkgsh::BUILD_DATA;

static CONFIG_OPT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<opt>--[\w\+_\.-]+)(=(?P<val>\w+))?").unwrap());

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

    // convert args into an indexed set so they can be easily merged with the default options
    let args: IndexMap<&str, Option<String>> = args
        .iter()
        .map(|&s| {
            s.split_once('=')
                .map_or_else(|| (s, None), |(o, v)| (o, Some(v.to_string())))
        })
        .collect();

    // parse `./configure --help` output to determine supported options
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

        if !args.contains_key("--libdir") {
            if let Some(libdir) = get_libdir(None) {
                let prefix = match args.get("--exec-prefix") {
                    Some(Some(v)) => v.clone(),
                    _ => match args.get("--prefix") {
                        Some(Some(v)) => v.clone(),
                        _ => format!("{}/usr", eprefix),
                    },
                };
                defaults.insert("--libdir", Some(format!("{}/{}", prefix, libdir)));
            }
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

    // merge args over default options and then add them as command args
    let mut econf = Command::new(&configure);
    defaults.extend(args);
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
    use std::io::{prelude::*, Read};
    use std::process::{Command, Stdio};

    use gag::BufferRedirect;
    use indexmap::IndexSet;
    use indoc::indoc;
    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    use super::{BUILTIN as econf, CONFIG_OPT_RE};
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    static CONFIGURE_AC: &str = indoc! {"
        AC_INIT([pkgcraft], [0.0.1], [pkgcraft@pkgcraft.org])
        AM_INIT_AUTOMAKE([-Wall -Werror foreign])
        LT_INIT
        AC_PROG_CC
        AC_OUTPUT
    "};

    rusty_fork_test! {
        #[test]
        fn nonexistent() {
            assert_err_re!(econf.run(&[]), format!("^nonexistent configure .*$"));
        }

        #[test]
        fn nonexecutable() {
            let dir = tempdir().unwrap();
            let configure = dir.path().join("configure");
            File::create(configure).unwrap();
            env::set_current_dir(&dir).unwrap();
            assert_err_re!(econf.run(&[]), format!("^nonexecutable configure .*$"));
        }

        #[test]
        #[cfg_attr(target_os = "macos", ignore)]
        fn args() {
            let dir = tempdir().unwrap();
            let configure_ac = dir.path().join("configure.ac");
            let file = File::create(configure_ac).unwrap();
            env::set_current_dir(&dir).unwrap();
            write!(&file, "{}", CONFIGURE_AC).unwrap();
            Command::new("autoreconf").arg("-i").stderr(Stdio::null()).status().unwrap();

            let mut buf = BufferRedirect::stdout().unwrap();
            let mut run = |args: &[&str]| -> (Vec<String>, IndexSet<String>) {
                // TODO: Mock out command call in some fashion to capture params instead of
                // actually running the configure script.
                econf.run(args).unwrap();
                let mut output = String::new();
                buf.read_to_string(&mut output).unwrap();
                let output: Vec<&str> = output.split('\n').collect();
                let cmd: Vec<String> = output[0].split(' ').map(|s| s.to_string()).collect();
                let mut opts = IndexSet::<String>::new();
                for param in &cmd[1..] {
                    for caps in CONFIG_OPT_RE.captures_iter(param) {
                        opts.insert(caps["opt"].to_string());
                    }
                }
                (cmd, opts)
            };

            BUILD_DATA.with(|d| {
                // TODO: add support for generating build state data for tests
                d.borrow_mut().env.extend([
                    ("EPREFIX".into(), "/eprefix".into()),
                    ("CHOST".into(), "x86_64-pc-linux-gnu".into()),
                ]);

                // verify EAPI specific options are added
                for eapi in econf.scope.keys() {
                    d.borrow_mut().eapi = eapi;
                    if !eapi.econf_options().is_empty() {
                        let (_cmd, opts) = run(&[]);
                        let eapi_opts: Vec<&String> = eapi.econf_options().keys().collect();
                        let cmd_opts: Vec<&String> = opts.iter().collect();
                        assert_eq!(&eapi_opts, &cmd_opts[cmd_opts.len()-eapi_opts.len()..]);
                    }
                }

                for arg in ["--prefix=/dir", "CC=gcc"] {
                    let (cmd, _opts) = run(&[arg]);
                    assert!(cmd.contains(&arg.to_string()), "command missing argument: {}", arg);
                }
            });
        }
    }
}
