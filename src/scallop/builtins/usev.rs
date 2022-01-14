use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::{Error, Result};

use super::r#use;
use crate::scallop::BUILD_DATA;

static LONG_DOC: &str = "\
The same as use, but also prints the flag name if the condition is met.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (arg, output) = match args.len() {
            1 => {
                let flag = args[0].strip_prefix('!').unwrap_or(args[0]);
                (args[0], flag)
            }
            2 => match eapi.has("usev_two_args") {
                true => (args[0], args[1]),
                false => return Err(scallop::Error::new("requires 1 arg, got 2")),
            },
            n => return Err(Error::new(format!("requires 1 or 2 args, got {}", n))),
        };

        let ret = r#use::run(&[arg])?;
        if bool::from(&ret) {
            println!("{}", output);
        }

        Ok(ret)
    })
}

pub static BUILTIN: Builtin = Builtin {
    name: "usev",
    func: run,
    help: LONG_DOC,
    usage: "usev flag",
    error_func: Some(output_error_func),
};
