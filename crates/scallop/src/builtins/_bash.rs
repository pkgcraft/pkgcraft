use crate::command::cmd_scope;
use crate::error::{ok_or_error, Error};
use crate::traits::*;
use crate::{bash, ExecStatus};

/// Run the `declare` builtin with the given arguments.
pub fn declare<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let args = Words::from_iter(args);
    ok_or_error(|| {
        cmd_scope("declare", || {
            let ret = unsafe { bash::declare_builtin((&args).into()) };
            if ret == 0 {
                Ok(ExecStatus::Success)
            } else {
                Err(Error::Base(format!(
                    "failed running declare builtin: exit status {}",
                    ret
                )))
            }
        })
    })
}

/// Run the `local` builtin with the given arguments.
pub fn local<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let args = Words::from_iter(args);
    ok_or_error(|| {
        cmd_scope("local", || {
            let ret = unsafe { bash::local_builtin((&args).into()) };
            if ret == 0 {
                Ok(ExecStatus::Success)
            } else {
                Err(Error::Base(format!("failed running local builtin: exit status {}", ret)))
            }
        })
    })
}

/// Run the `set` builtin with the given arguments.
pub fn set<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let args = Words::from_iter(args);
    ok_or_error(|| {
        cmd_scope("set", || {
            let ret = unsafe { bash::set_builtin((&args).into()) };
            if ret == 0 {
                Ok(ExecStatus::Success)
            } else {
                Err(Error::Base(format!("failed running set builtin: exit status {}", ret)))
            }
        })
    })
}

/// Run the `shopt` builtin with the given arguments.
pub fn shopt<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let args = Words::from_iter(args);
    ok_or_error(|| {
        cmd_scope("shopt", || {
            let ret = unsafe { bash::shopt_builtin((&args).into()) };
            if ret == 0 {
                Ok(ExecStatus::Success)
            } else {
                Err(Error::Base(format!("failed running shopt builtin: exit status {}", ret)))
            }
        })
    })
}

/// Run the `source` builtin with the given arguments.
pub fn source<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let args = Words::from_iter(args);
    ok_or_error(|| {
        cmd_scope("source", || {
            let ret = unsafe { bash::source_builtin((&args).into()) };
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
        // verify local function variable scope
        bind("VAR", "outer", None, None).unwrap();
        bash_func("func_name", || {
            let result = local(["VAR=inner"]);
            assert_eq!(optional("VAR").unwrap(), "inner");
            result
        })
        .unwrap();
        assert_eq!(optional("VAR").unwrap(), "outer");

        // local doesn't work in global scope
        assert!(local(["VAR=inner"]).is_err());
    }
}
