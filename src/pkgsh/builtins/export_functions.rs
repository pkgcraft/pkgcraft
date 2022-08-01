use scallop::builtins::ExecStatus;
use scallop::variables::string_value;
use scallop::{source, Error, Result};

use super::{make_builtin, ECLASS};

const LONG_DOC: &str = "\
Export stub functions that call the eclass's functions, thereby making them default.
For example, if ECLASS=base and `EXPORT_FUNCTIONS src_unpack` is called the following
function is defined:

src_unpack() { base_src_unpack; }";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    let eclass =
        string_value("ECLASS").ok_or_else(|| Error::Builtin("no ECLASS defined".into()))?;

    // TODO: verify phase function existence?
    let funcs: Vec<_> = args
        .iter()
        .map(|func| format!("{func}() {{ {eclass}_{func} \"$@\"; }}", func = func, eclass = eclass))
        .collect();

    source::string(funcs.join("\n"))
}

const USAGE: &str = "EXPORT_FUNCTIONS src_configure src_compile";
make_builtin!(
    "EXPORT_FUNCTIONS",
    export_functions_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("0-", &[ECLASS])]
);

#[cfg(test)]
mod tests {
    use std::fs;

    use scallop::functions;
    use scallop::variables::bind;

    use crate::config::Config;
    use crate::pkgsh::{source_ebuild, BUILD_DATA};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as export_functions;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(export_functions, &[0]);
    }

    #[test]
    fn test_single() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        bind("TEMP_FILE", temp_file.path().to_string_lossy(), None, None).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            EXPORT_FUNCTIONS src_compile

            e1_src_compile() {
                echo compiling >> ${TEMP_FILE}
            }
        "#};
        t.create_eclass("e1", eclass).unwrap();

        let data = indoc::indoc! {r#"
            inherit e1
            DESCRIPTION="testing EXPORT_FUNCTIONS support"
            SLOT=0
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            source_ebuild(&path).unwrap();
            // execute eclass-defined function
            let mut func = functions::find("src_compile").unwrap();
            func.execute(&[]).unwrap();
            // verify the function was run
            let output = fs::read_to_string(temp_file.path()).unwrap();
            let output: Vec<_> = output.split_whitespace().collect();
            assert_eq!(output, ["compiling"]);
        });
    }
}
