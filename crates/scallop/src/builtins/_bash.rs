use crate::bash;
use crate::builtins::ExecStatus;
use crate::command::cmd_scope;
use crate::error::{ok_or_error, Error};
use crate::traits::*;

/// Run the `declare` builtin with the given arguments.
pub fn declare(args: &[&str]) -> crate::Result<ExecStatus> {
    let args = Words::from_iter(args.iter().copied());
    ok_or_error(|| {
        cmd_scope("declare", || unsafe {
            let ret = bash::declare_builtin((&args).into());
            if ret == 0 {
                Ok(ExecStatus::Success)
            } else {
                Err(Error::Base(format!("failed running declare builtin: exit status {}", ret)))
            }
        })
    })
}

/// Run the `local` builtin with the given arguments.
pub fn local(args: &[&str]) -> crate::Result<ExecStatus> {
    let args = Words::from_iter(args.iter().copied());
    ok_or_error(|| {
        cmd_scope("local", || unsafe {
            let ret = bash::local_builtin((&args).into());
            if ret == 0 {
                Ok(ExecStatus::Success)
            } else {
                Err(Error::Base(format!("failed running local builtin: exit status {}", ret)))
            }
        })
    })
}

/// Run the `set` builtin with the given arguments.
pub fn set(args: &[&str]) -> crate::Result<ExecStatus> {
    let args = Words::from_iter(args.iter().copied());
    ok_or_error(|| {
        cmd_scope("set", || unsafe {
            let ret = bash::set_builtin((&args).into());
            if ret == 0 {
                Ok(ExecStatus::Success)
            } else {
                Err(Error::Base(format!("failed running set builtin: exit status {}", ret)))
            }
        })
    })
}

/// Run the `shopt` builtin with the given arguments.
pub fn shopt(args: &[&str]) -> crate::Result<ExecStatus> {
    let args = Words::from_iter(args.iter().copied());
    ok_or_error(|| {
        cmd_scope("shopt", || unsafe {
            let ret = bash::shopt_builtin((&args).into());
            if ret == 0 {
                Ok(ExecStatus::Success)
            } else {
                Err(Error::Base(format!("failed running shopt builtin: exit status {}", ret)))
            }
        })
    })
}

/// Run the `source` builtin with the given arguments.
pub fn source(args: &[&str]) -> crate::Result<ExecStatus> {
    let args = Words::from_iter(args.iter().copied());
    ok_or_error(|| {
        cmd_scope("source", || unsafe {
            let ret = bash::source_builtin((&args).into());
            if ret == 0 {
                Ok(ExecStatus::Success)
            } else {
                Err(Error::Base(format!("failed running source builtin: exit status {}", ret)))
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use crate::functions::bash_func;
    use crate::variables::{bind, optional};

    use super::*;

    #[test]
    fn test_local() {
        bind("VAR", "outer", None, None).unwrap();
        bash_func("func_name", || {
            local(&["VAR=inner"]).unwrap();
            assert_eq!(optional("VAR").unwrap(), "inner");
        });
        assert_eq!(optional("VAR").unwrap(), "outer");
    }
}
