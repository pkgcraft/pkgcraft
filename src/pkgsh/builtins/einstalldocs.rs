use std::fs;

use glob::glob;
use scallop::builtins::ExecStatus;
use scallop::variables::var_to_vec;
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

use super::{dodoc::run as dodoc, make_builtin};

const LONG_DOC: &str = "\
Installs the files specified by the DOCS and HTML_DOCS variables or a default set of files.";

const DOCS_DEFAULTS: &[&str] = &[
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
// TODO: replace glob usage with native bash pathname expansion?
// TODO: need to perform word expansion on each string as well
fn expand_docs<S: AsRef<str>>(globs: &[S]) -> Result<Vec<String>> {
    let mut args = vec![];
    // TODO: output warnings for unmatched patterns when running against non-default input
    for f in globs.iter() {
        let paths = glob(f.as_ref()).map_err(|e| Error::Builtin(e.to_string()))?;
        for path in paths.flatten() {
            let m = fs::metadata(&path).map_err(|e| Error::Builtin(e.to_string()))?;
            if m.len() > 0 {
                args.push(path.to_str().unwrap().to_string());
            }
        }
    }
    Ok(args)
}

pub(crate) fn install_docs(var: &str) -> Result<ExecStatus> {
    let (defaults, docdesttree) = match var {
        "DOCS" => (Some(DOCS_DEFAULTS), ""),
        "HTML_DOCS" => (None, "html"),
        _ => return Err(Error::Builtin(format!("unknown variable: {var}"))),
    };

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let (mut args, files) = match var_to_vec(var) {
            Ok(v) => (vec!["-r"], expand_docs(&v)?),
            _ => match defaults {
                Some(v) => (vec![], expand_docs(v)?),
                None => (vec![], vec![]),
            },
        };

        if !files.is_empty() {
            // save original docdesttree value and use custom value
            let orig_docdestree = d.borrow().docdesttree.clone();
            d.borrow_mut().docdesttree = String::from(docdesttree);

            args.extend(files.iter().map(|s| s.as_str()));
            dodoc(&args)?;

            // restore original docdesttree value
            d.borrow_mut().docdesttree = orig_docdestree;
        }

        Ok(ExecStatus::Success)
    })
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Builtin(format!("takes no args, got {}", args.len())));
    }

    for var in ["DOCS", "HTML_DOCS"] {
        install_docs(var)?;
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "einstalldocs";
make_builtin!(
    "einstalldocs",
    einstalldocs_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("6-", &["src_install"])]
);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as einstalldocs;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(einstalldocs, &[1]);
    }

    // TODO: add usage tests
}
