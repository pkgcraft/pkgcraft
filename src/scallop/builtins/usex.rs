use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::{Error, Result};

use super::r#use;

static LONG_DOC: &str = "\
Returns --with-${opt} and --without-${opt} configure flags based on a given USE flag.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let defaults = ["", "yes", "no", "", ""];
    let (flag, vals) = match args.len() {
        1 => (args[0], defaults),
        2..=5 => {
            // override default values with args
            let stop = args.len();
            let mut vals = defaults;
            vals[1..stop].clone_from_slice(&args[1..stop]);
            (args[0], vals)
        }
        n => return Err(Error::new(format!("requires 1 to 5 args, got {}", n))),
    };

    match r#use::run(&[flag])? {
        ExecStatus::Success => println!("{}{}", vals[1], vals[3]),
        ExecStatus::Failure => println!("{}{}", vals[2], vals[4]),
    }

    Ok(ExecStatus::Success)
}

pub static BUILTIN: Builtin = Builtin {
    name: "use_with",
    func: run,
    help: LONG_DOC,
    usage: "use_with flag",
    error_func: Some(output_error_func),
};
