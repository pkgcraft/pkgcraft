use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::sync::LazyLock;

use indexmap::{IndexMap, IndexSet};
use is_executable::IsExecutable;
use regex::Regex;
use scallop::{Error, ExecStatus, variables};

use crate::command::RunCommand;
use crate::io::stdout;
use crate::shell::get_build_mut;
use crate::shell::utils::{configure, get_libdir};

use super::{TryParseArgs, make_builtin};

#[derive(Debug, Clone)]
pub(crate) struct EconfOption {
    option: String,
    markers: IndexSet<String>,
    value: Option<String>,
}

impl PartialEq for EconfOption {
    fn eq(&self, other: &Self) -> bool {
        self.option == other.option
    }
}

impl Eq for EconfOption {}

impl Hash for EconfOption {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.option.hash(state);
    }
}

impl Ord for EconfOption {
    fn cmp(&self, other: &Self) -> Ordering {
        self.option.cmp(&other.option)
    }
}

impl PartialOrd for EconfOption {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl EconfOption {
    /// Create a new econf option.
    pub(crate) fn new(option: &str) -> Self {
        Self {
            option: option.to_string(),
            markers: [option.to_string()].into_iter().collect(),
            value: None,
        }
    }

    /// Add custom options to match for an econf option to be applied.
    pub(crate) fn markers<I>(mut self, values: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        self.markers.extend(values.into_iter().map(Into::into));
        self
    }

    /// Set the value for an econf option.
    pub(crate) fn value(mut self, value: &str) -> Self {
        self.value = Some(value.to_string());
        self
    }

    /// Expand the value of an econf option.
    fn expand(&self) -> Option<String> {
        self.value.as_ref().and_then(variables::expand)
    }
}

static CONFIG_OPT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<opt>--[\w\+_\.-]+)").unwrap());

#[derive(clap::Parser, Debug)]
#[command(
    name = "econf",
    disable_help_flag = true,
    long_about = "Run a package's configure script."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(allow_hyphen_values = true)]
    args: Vec<String>,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;

    // verify configure scripts is executable
    let configure = configure();
    if !configure.is_executable() {
        let msg = if configure.exists() {
            "nonexecutable configure script"
        } else {
            "nonexistent configure script"
        };
        return Err(Error::Base(msg.to_string()));
    }

    // convert args to options mapping
    let args: Vec<_> = cmd.args.iter().map(|x| x.as_str()).collect();
    let args: IndexMap<_, _> = args
        .iter()
        .map(|&s| {
            s.split_once('=')
                .map_or_else(|| (s, None), |(o, v)| (o, Some(v.to_string())))
        })
        .collect();

    // parse `./configure --help` output to determine supported options
    let conf_help = std::process::Command::new(&configure)
        .arg("--help")
        .output()
        .map_err(|e| Error::Base(format!("failed running: {e}")))?;
    let known_opts: IndexSet<_> = String::from_utf8_lossy(&conf_help.stdout)
        .lines()
        .flat_map(|line| CONFIG_OPT_RE.captures_iter(line.trim()))
        .map(|cap| cap["opt"].to_string())
        .collect();

    let eprefix = variables::required("EPREFIX")?;
    let chost = variables::required("CHOST")?;

    // set default options
    let mut options: IndexMap<_, _> = [
        ("--prefix", Some(format!("{eprefix}/usr"))),
        ("--mandir", Some(format!("{eprefix}/usr/share/man"))),
        ("--infodir", Some(format!("{eprefix}/usr/share/info"))),
        ("--datadir", Some(format!("{eprefix}/usr/share"))),
        ("--sysconfdir", Some(format!("{eprefix}/etc"))),
        ("--localstatedir", Some(format!("{eprefix}/var/lib"))),
        ("--host", Some(chost)),
    ]
    .into_iter()
    .collect();

    // set libdir if unspecified
    if !args.contains_key("--libdir")
        && let Some(libdir) = get_libdir(None)
    {
        let value = if let Some(Some(prefix)) = args.get("--exec-prefix") {
            format!("{prefix}/{libdir}")
        } else if let Some(Some(prefix)) = args.get("--prefix") {
            format!("{prefix}/{libdir}")
        } else {
            format!("{eprefix}/usr/{libdir}")
        };
        options.insert("--libdir", Some(value));
    }

    // inject cross-compile options if enabled
    for (opt, var) in [("--build", "CBUILD"), ("--target", "CTARGET")] {
        if let Some(val) = variables::optional(var) {
            options.insert(opt, Some(val));
        }
    }

    // inject EAPI options
    for opt in get_build_mut().eapi().econf_options() {
        if !known_opts.is_disjoint(&opt.markers) {
            options.insert(&opt.option, opt.expand());
        }
    }

    // override default options with args
    options.extend(args);

    // add options as command args
    let mut econf = std::process::Command::new(&configure);
    for (opt, val) in options {
        if let Some(value) = val {
            econf.arg(format!("{opt}={value}"));
        } else {
            econf.arg(opt);
        }
    }

    // run configure script
    write!(stdout(), "{}", econf.to_vec().join(" "))?;
    econf.run()?;

    Ok(ExecStatus::Success)
}

make_builtin!("econf", econf_builtin);

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs::File;

    use scallop::variables::{ScopedVariable, ShellVariable, bind, unbind};
    use tempfile::tempdir;

    use crate::command::commands;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::macros::build_path;
    use crate::shell::phase::PhaseKind::SrcConfigure;
    use crate::shell::{BuildData, Scope};
    use crate::test::assert_err_re;

    use super::super::{cmd_scope_tests, functions::econf};
    use super::*;

    cmd_scope_tests!("econf --enable-feature");

    fn get_opts(args: &[&str]) -> IndexMap<String, Option<String>> {
        econf(args).unwrap();
        let cmd = commands().pop().unwrap();
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
        get_build_mut().scope = Scope::Phase(SrcConfigure);
        assert_err_re!(econf(&[]), "^nonexistent configure .*$");
    }

    #[test]
    fn nonexecutable() {
        get_build_mut().scope = Scope::Phase(SrcConfigure);
        let dir = tempdir().unwrap();
        let configure = dir.path().join("configure");
        File::create(configure).unwrap();
        env::set_current_dir(&dir).unwrap();
        assert_err_re!(econf(&[]), "^nonexecutable configure .*$");
    }

    #[test]
    fn args() {
        get_build_mut().scope = Scope::Phase(SrcConfigure);
        let configure_dir = build_path!(env!("CARGO_MANIFEST_DIR"), "testdata", "autotools");
        env::set_current_dir(configure_dir).unwrap();

        // TODO: add support for generating build state data for tests
        bind("EPREFIX", "/eprefix", None, None).unwrap();
        bind("CHOST", "x86_64-pc-linux-gnu", None, None).unwrap();
        bind("PF", "pkg-1", None, None).unwrap();

        // force libdir default
        bind("ABI", "arch", None, None).unwrap();
        unbind("LIBDIR_arch").unwrap();

        // verify EAPI specific options are added
        for eapi in &*EAPIS_OFFICIAL {
            BuildData::empty(eapi);
            if !eapi.econf_options().is_empty() {
                let opts = get_opts(&[]);
                let eapi_opts: Vec<_> =
                    eapi.econf_options().iter().map(|x| &x.option).collect();
                let cmd_opts: Vec<_> = opts.keys().map(|s| s.as_str()).collect();
                assert_eq!(
                    &eapi_opts,
                    &cmd_opts[cmd_opts.len() - eapi_opts.len()..],
                    "EAPI {eapi} failed"
                );
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
    }
}
