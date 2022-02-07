use std::fs;

use glob::glob;
use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::variables::var_to_vec;
use scallop::{Error, Result};

use super::dodoc;
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "\
Installs the files specified by the DOCS and HTML_DOCS variables or a default set of files.";

static DOC_DEFAULTS: &[&str] = &[
    "README*",
    "ChangeLog",
    "AUTHORS",
    "NEWS",
    "TODO",
    "CHANGES",
    "THANKS",
    "BUGS",
    "FAQ",
    "CREDITS",
    "CHANGELOG",
];

// Perform file expansion on doc strings.
fn expand_docs<S: AsRef<str>>(globs: &[S]) -> Result<Vec<String>> {
    let mut args: Vec<String> = vec![];
    // TODO: output warnings for unmatched patterns when running against non-default input
    for f in globs.iter() {
        let paths = glob(f.as_ref()).map_err(|e| Error::new(e.to_string()))?;
        for path in paths.flatten() {
            let m = fs::metadata(&path).map_err(|e| Error::new(e.to_string()))?;
            if m.len() > 0 {
                args.push(path.to_str().unwrap().to_string());
            }
        }
    }
    Ok(args)
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    match args.len() {
        0 => (),
        n => return Err(Error::new(format!("takes no args, got {}", n))),
    };

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        // save original docdesttree value
        let orig_docdestree = d.borrow().docdesttree.clone();

        for (var, defaults, docdesttree) in [
            ("DOCS", Some(DOC_DEFAULTS), ""),
            ("HTML_DOCS", None, "html"),
        ] {
            let (opts, files) = match var_to_vec(var) {
                Ok(v) => (vec!["-r"], expand_docs(v.as_slice())?),
                _ => match defaults {
                    Some(v) => (vec![], expand_docs(v)?),
                    None => continue,
                },
            };

            if !files.is_empty() {
                d.borrow_mut().docdesttree = String::from(docdesttree);
                let mut args: Vec<&str> = opts;
                args.extend(files.iter().map(|s| s.as_str()));
                dodoc::run(args.as_slice())?;
            }
        }

        // restore original docdesttree value
        d.borrow_mut().docdesttree = orig_docdestree;

        Ok(ExecStatus::Success)
    })
}

pub static BUILTIN: Builtin = Builtin {
    name: "einstalldocs",
    func: run,
    help: LONG_DOC,
    usage: "einstalldocs",
    error_func: Some(output_error_func),
};

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as einstalldocs;

    #[test]
    fn invalid_args() {
        assert_invalid_args(einstalldocs, &[1]);
    }
}
