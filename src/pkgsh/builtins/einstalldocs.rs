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

fn filter_docs(globs: &[&str]) -> Result<Vec<String>> {
    let mut args: Vec<String> = vec![];
    for f in globs.iter() {
        for entry in glob(f).map_err(|e| Error::new(e.to_string()))? {
            if let Ok(path) = entry {
                let m = fs::metadata(&path).map_err(|e| Error::new(e.to_string()))?;
                if m.len() > 0 {
                    args.push(path.to_str().unwrap().to_string());
                }
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
            let args = match var_to_vec(var) {
                Ok(v) => {
                    let mut args: Vec<String> = vec!["-r".to_string()];
                    args.extend(v);
                    args
                }
                _ => match defaults {
                    Some(v) => filter_docs(v)?,
                    None => continue,
                },
            };
            d.borrow_mut().docdesttree = String::from(docdesttree);
            let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            dodoc::run(args.as_slice())?;
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
        assert_invalid_args(einstalldocs, vec![1]);
    }
}
