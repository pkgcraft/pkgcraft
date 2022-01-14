use scallop::builtins::{output_error_func, Builtin};
use scallop::{Error, Result};

use super::r#use;

static LONG_DOC: &str = "\
Returns --with-${opt} and --without-${opt} configure flags based on a given USE flag.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<i32> {
    let (flag, vals) = match args.len() {
        1..=5 => {
            let mut vals = ["", "yes", "no", "", ""];
            for idx in (args.len() - 1)..=4 {
                vals[idx] = args[idx];
            }
            (args[0], vals)
        }
        n => return Err(Error::new(format!("requires 1 to 5 args, got {}", n))),
    };

    let ret = r#use::run(&[flag])?;
    match ret {
        0 => println!("{}{}", vals[1], vals[3]),
        1 => println!("{}{}", vals[2], vals[4]),
        n => panic!("invalid return value: {}", n),
    }

    Ok(0)
}

pub static BUILTIN: Builtin = Builtin {
    name: "use_with",
    func: run,
    help: LONG_DOC,
    usage: "use_with flag",
    error_func: Some(output_error_func),
};
