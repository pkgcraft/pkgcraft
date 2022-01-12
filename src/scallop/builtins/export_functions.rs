use scallop::builtins::{output_error_func, Builtin};
use scallop::variables::string_value;
use scallop::{source, Error, Result};

static LONG_DOC: &str = "\
Export stub functions that call the eclass's functions, thereby making them default.
For example, if ECLASS=base and `EXPORT_FUNCTIONS src_unpack` is called the following
function is defined:

src_unpack() { base_src_unpack; }";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<i32> {
    let eclass = match string_value("ECLASS") {
        Some(val) => val,
        None => return Err(Error::new("no ECLASS defined")),
    };

    let funcs: Vec<String> = args
        .iter()
        .map(|func| {
            format!(
                "{func}() {{ {eclass}_{func} \"$@\"; }}",
                func = func,
                eclass = eclass
            )
        })
        .collect();

    source::string(funcs.join("\n"))?;

    Ok(0)
}

pub static BUILTIN: Builtin = Builtin {
    name: "EXPORT_FUNCTIONS",
    func: run,
    help: LONG_DOC,
    usage: "EXPORT_FUNCTIONS src_configure src_compile",
    error_func: Some(output_error_func),
};
