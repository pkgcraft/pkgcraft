use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::variables::string_value;
use scallop::{source, Error, Result};

use super::{PkgBuiltin, ECLASS};

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

    let eclass = match string_value("ECLASS") {
        Some(val) => val,
        None => return Err(Error::Builtin("no ECLASS defined".into())),
    };

    let funcs: Vec<String> = args
        .iter()
        .map(|func| format!("{func}() {{ {eclass}_{func} \"$@\"; }}", func = func, eclass = eclass))
        .collect();

    source::string(funcs.join("\n"))?;

    Ok(ExecStatus::Success)
}

make_builtin!(
    "EXPORT_FUNCTIONS",
    export_functions_builtin,
    run,
    LONG_DOC,
    "EXPORT_FUNCTIONS src_configure src_compile"
);

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &[ECLASS])]));

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as inherit;

    #[test]
    fn invalid_args() {
        assert_invalid_args(inherit, &[0]);
    }
}
