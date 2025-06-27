use crate::error::{Error, ok_or_error};
use crate::traits::Words;
use crate::{ExecStatus, bash};

/// Run the `declare` builtin with the given arguments.
pub fn declare<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let args: Words = [String::from("declare")]
        .into_iter()
        .chain(args.into_iter().map(Into::into))
        .collect();
    ok_or_error(|| unsafe {
        let ret = bash::builtin_builtin(args.as_ptr());
        if ret == 0 {
            Ok(ExecStatus::Success)
        } else {
            Err(Error::Base(format!("failed running declare builtin: exit status {ret}")))
        }
    })
}

/// Run the `local` builtin with the given arguments.
pub fn local<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let args: Words = [String::from("local")]
        .into_iter()
        .chain(args.into_iter().map(Into::into))
        .collect();
    ok_or_error(|| unsafe {
        let ret = bash::builtin_builtin(args.as_ptr());
        if ret == 0 {
            Ok(ExecStatus::Success)
        } else {
            Err(Error::Base(format!("failed running local builtin: exit status {ret}")))
        }
    })
}

/// Run the `set` builtin with the given arguments.
pub fn set<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let args: Words = [String::from("set")]
        .into_iter()
        .chain(args.into_iter().map(Into::into))
        .collect();
    ok_or_error(|| unsafe {
        let ret = bash::builtin_builtin(args.as_ptr());
        if ret == 0 {
            Ok(ExecStatus::Success)
        } else {
            Err(Error::Base(format!("failed running set builtin: exit status {ret}")))
        }
    })
}

/// Run the `shopt` builtin with the given arguments.
pub fn shopt<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let args: Words = [String::from("shopt")]
        .into_iter()
        .chain(args.into_iter().map(Into::into))
        .collect();
    ok_or_error(|| unsafe {
        let ret = bash::builtin_builtin(args.as_ptr());
        if ret == 0 {
            Ok(ExecStatus::Success)
        } else {
            Err(Error::Base(format!("failed running shopt builtin: exit status {ret}")))
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::functions::bash_func;
    use crate::variables::{bind, optional};

    #[test]
    fn declare() {
        // invalid args
        assert!(super::declare(["-Z", "foo"]).is_err());

        // valid args
        assert!(super::declare(["-a", "foo"]).is_ok());
    }

    #[test]
    fn local() {
        // verify local function variable scope
        bind("VAR", "outer", None, None).unwrap();
        bash_func("func_name", || {
            let result = super::local(["VAR=inner"]);
            assert_eq!(optional("VAR").unwrap(), "inner");
            result
        })
        .unwrap();
        assert_eq!(optional("VAR").unwrap(), "outer");

        // local doesn't work in global scope
        assert!(super::local(["VAR=inner"]).is_err());
    }

    #[test]
    fn set() {
        // invalid args
        assert!(super::set(["-o", "foo"]).is_err());

        // valid args
        assert!(super::set(["-o", "errexit"]).is_ok());
        assert!(super::set(["+e"]).is_ok());
    }

    #[test]
    fn shopt() {
        // invalid args
        assert!(super::shopt(["-s", "foo"]).is_err());

        // valid args
        assert!(super::shopt(["-s", "failglob"]).is_ok());
        assert!(super::shopt(["-u", "failglob"]).is_ok());
    }
}
