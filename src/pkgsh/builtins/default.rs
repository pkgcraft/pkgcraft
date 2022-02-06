use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::{functions, Error, Result};

use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Calls the default_* function for the current phase.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    match args.len() {
        0 => (),
        n => return Err(Error::new(format!("takes no args, got {}", n))),
    };

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let func_name = format!("default_{}", d.borrow().phase_func);
        if let Some(mut func) = functions::find(&func_name) {
            func.execute(&[])?;
        }
        Ok(ExecStatus::Success)
    })
}

pub static BUILTIN: Builtin = Builtin {
    name: "default",
    func: run,
    help: LONG_DOC,
    usage: "default",
    error_func: Some(output_error_func),
};

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as default;

    #[test]
    fn invalid_args() {
        assert_invalid_args(default, vec![1]);
    }
}
