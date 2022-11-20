use std::io::Write;
use std::process::Command;

use indexmap::{IndexMap, IndexSet};
use is_executable::IsExecutable;
use regex::Regex;
use scallop::builtins::ExecStatus;
use scallop::{variables, Error, Result};

use crate::command::RunCommand;
use crate::pkgsh::utils::{configure, get_libdir};
use crate::pkgsh::write_stdout;
use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

static CONFIG_OPT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(?P<opt>--[\w\+_\.-]+)").unwrap());
const LONG_DOC: &str = "Run a package's configure script.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let configure = configure();
    if !configure.is_executable() {
        let msg = match configure.exists() {
            true => "nonexecutable configure script",
            false => "nonexistent configure script",
        };
        return Err(Error::Base(msg.to_string()));
    }

    // convert args into an indexed set so they can be easily merged with the default options
    let args: IndexMap<&str, Option<String>> = args
        .iter()
        .map(|&s| {
            s.split_once('=')
                .map_or_else(|| (s, None), |(o, v)| (o, Some(v.into())))
        })
        .collect();

    // parse `./configure --help` output to determine supported options
    let conf_help = Command::new(&configure)
        .arg("--help")
        .output()
        .map_err(|e| Error::Base(format!("failed running: {e}")))?;
    let mut known_opts = IndexSet::<String>::new();
    let conf_help = String::from_utf8_lossy(&conf_help.stdout);
    for line in conf_help.split('\n') {
        for caps in CONFIG_OPT_RE.captures_iter(line.trim()) {
            known_opts.insert(caps["opt"].to_string());
        }
    }

    let mut defaults = IndexMap::<&str, Option<String>>::new();
    let eprefix = variables::required("EPREFIX")?;
    let chost = variables::required("CHOST")?;

    defaults.extend([
        ("--prefix", Some(format!("{eprefix}/usr"))),
        ("--mandir", Some(format!("{eprefix}/usr/share/man"))),
        ("--infodir", Some(format!("{eprefix}/usr/share/info"))),
        ("--datadir", Some(format!("{eprefix}/usr/share"))),
        ("--sysconfdir", Some(format!("{eprefix}/etc"))),
        ("--localstatedir", Some(format!("{eprefix}/var/lib"))),
        ("--host", Some(chost)),
    ]);

    if !args.contains_key("--libdir") {
        if let Some(libdir) = get_libdir(None) {
            let prefix = match args.get("--exec-prefix") {
                Some(Some(v)) => v.clone(),
                _ => match args.get("--prefix") {
                    Some(Some(v)) => v.clone(),
                    _ => format!("{eprefix}/usr"),
                },
            };
            defaults.insert("--libdir", Some(format!("{prefix}/{libdir}")));
        }
    }

    for (opt, var) in [("--build", "CBUILD"), ("--target", "CTARGET")] {
        if let Some(val) = variables::optional(var) {
            defaults.insert(opt, Some(val));
        }
    }

    BUILD_DATA.with(|d| {
        // add EAPI specific options if found
        for (opt, (markers, val)) in d.borrow().eapi.econf_options() {
            if !known_opts.is_disjoint(markers) {
                defaults.insert(opt, val.as_ref().and_then(variables::expand));
            }
        }
    });

    // merge args over default options and then add them as command args
    let mut econf = Command::new(&configure);
    defaults.extend(args);
    for (opt, val) in defaults.iter() {
        match val {
            None => econf.arg(opt),
            Some(v) => econf.arg(format!("{opt}={v}")),
        };
    }

    write_stdout!("{}", econf.to_vec().join(" "));
    econf.run()?;
    Ok(ExecStatus::Success)
}

const USAGE: &str = "econf --enable-feature";
make_builtin!(
    "econf",
    econf_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("0-1", &["src_compile"]), ("2-", &["src_configure"])]
);

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs::File;

    use scallop::variables::{bind, ScopedVariable, Variables};
    use tempfile::tempdir;

    use crate::command::last_command;
    use crate::macros::{assert_err_re, build_from_paths};
    use crate::pkgsh::BUILD_DATA;

    use super::super::builtin_scope_tests;
    use super::PKG_BUILTIN as econf;
    use super::*;

    builtin_scope_tests!(USAGE);

    fn get_opts(args: &[&str]) -> IndexMap<String, Option<String>> {
        econf.run(args).unwrap();
        let cmd = last_command().unwrap();
        cmd[1..]
            .iter()
            .map(|s| {
                s.split_once('=')
                    .map_or_else(|| (s.into(), None), |(o, v)| (o.into(), Some(v.into())))
            })
            .collect()
    }

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
    fn args() {
        let configure_dir = build_from_paths!(env!("CARGO_MANIFEST_DIR"), "testdata", "autotools");
        env::set_current_dir(&configure_dir).unwrap();

        BUILD_DATA.with(|d| {
            // TODO: add support for generating build state data for tests
            bind("EPREFIX", "/eprefix", None, None).unwrap();
            bind("CHOST", "x86_64-pc-linux-gnu", None, None).unwrap();
            bind("PF", "pkg-1", None, None).unwrap();

            // verify EAPI specific options are added
            for eapi in econf.scope.keys() {
                d.borrow_mut().eapi = eapi;
                if !eapi.econf_options().is_empty() {
                    let opts = get_opts(&[]);
                    let eapi_opts: Vec<_> = eapi.econf_options().keys().cloned().collect();
                    let cmd_opts: Vec<_> = opts.keys().map(|s| s.as_str()).collect();
                    assert_eq!(&eapi_opts, &cmd_opts[cmd_opts.len() - eapi_opts.len()..]);
                }
            }

            // verify user args are respected
            for (opt, expected) in [("--prefix", "/dir"), ("--libdir", "/dir"), ("CC", "gcc")] {
                let opts = get_opts(&[&format!("{opt}={expected}")]);
                let val = opts.get(opt).unwrap().as_ref().unwrap();
                assert_eq!(val, expected);
            }

            // --libdir doesn't get passed if related variables are unset
            let opts = get_opts(&[]);
            assert!(opts.get("--libdir").is_none());

            // set required variables and verify --libdir
            for (abi, libdir) in [("amd64", "lib64"), ("x86", "lib")] {
                // TODO: load this data from test profiles
                let mut abi_var = ScopedVariable::new("ABI");
                let mut libdir_var = ScopedVariable::new(format!("LIBDIR_{abi}"));
                abi_var.bind(abi, None, None).unwrap();
                libdir_var.bind(libdir, None, None).unwrap();

                let opts = get_opts(&[]);
                let val = opts.get("--libdir").unwrap().as_ref().unwrap();
                assert_eq!(val, &format!("/eprefix/usr/{libdir}"));
            }
        });
    }
}
