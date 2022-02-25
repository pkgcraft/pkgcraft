use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use clap::Parser;
use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install HTML documentation files.";

const DEFAULT_FILE_EXTS: &[&str] = &["css", "gif", "htm", "html", "jpeg", "jpg", "js", "png"];

#[derive(Parser, Debug, Default)]
#[clap(name = "dohtml")]
struct Options {
    #[clap(short = 'r')]
    recursive: bool,
    #[clap(short = 'V')]
    verbose: bool,
    #[clap(short = 'A')]
    extra_allowed_file_exts: Vec<String>,
    #[clap(short = 'a')]
    allowed_file_exts: Vec<String>,
    #[clap(short = 'f')]
    allowed_files: Vec<String>,
    #[clap(short = 'x')]
    excluded_dirs: Vec<String>,
    #[clap(short = 'p')]
    doc_prefix: Option<String>,
    // file targets
    targets: Vec<String>,
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let opts = match Options::try_parse_from(&[&["dohtml"], args].concat()) {
        Ok(opts) => opts,
        Err(e) => return Err(Error::Builtin(format!("invalid args: {e}"))),
    };

    if opts.targets.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    let mut allowed_file_exts: HashSet<String> = match opts.allowed_file_exts.is_empty() {
        true => DEFAULT_FILE_EXTS.iter().map(|s| s.to_string()).collect(),
        false => opts.allowed_file_exts.into_iter().collect(),
    };

    allowed_file_exts.extend(opts.extra_allowed_file_exts);
    let excluded_dirs: HashSet<&Path> = opts.excluded_dirs.iter().map(Path::new).collect();
    let allowed_files: HashSet<String> = opts.allowed_files.into_iter().collect();

    // TODO: output info if verbose option is enabled

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();
        let subdir = match d.docdesttree.as_str() {
            "" => "html",
            val => val,
        };
        let dest: PathBuf = ["/usr/share/doc", d.env.get("PF").expect("$PF undefined"), subdir]
            .iter()
            .collect();
        let install = d.install().dest(&dest)?;

        let (dirs, files): (Vec<&Path>, Vec<&Path>) =
            opts.targets.iter().map(Path::new).partition(|p| p.is_dir());

        if !dirs.is_empty() {
            if opts.recursive {
                let dirs = dirs.iter().filter(|&d| !excluded_dirs.contains(d));
                install.from_dirs(dirs)?;
            } else {
                return Err(Error::Builtin(format!(
                    "trying to install directory as file: {:?}",
                    dirs[0]
                )));
            }
        }

        // determine if a file is allowed to be installed
        let allowed_file = |path: &Path| -> bool {
            let filename = path
                .file_name()
                .unwrap_or_else(|| OsStr::new(""))
                .to_str()
                .unwrap_or("");
            let ext = path
                .extension()
                .unwrap_or_else(|| OsStr::new(""))
                .to_str()
                .unwrap_or("");
            allowed_file_exts.contains(ext) || allowed_files.contains(filename)
        };

        let files = files
            .into_iter()
            .filter(|f| allowed_file(f))
            .filter_map(|f| f.file_name().map(|name| (f, name)));
        install.files(files)?;

        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "dohtml",
            func: run,
            help: LONG_DOC,
            usage: "dohtml path/to/html/files",
        },
        &[("0-6", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as dohtml;

    #[test]
    fn invalid_args() {
        assert_invalid_args(dohtml, &[0]);
    }
}
