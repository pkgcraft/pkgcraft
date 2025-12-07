use crate::ExecStatus;
use crate::builtins::BashBuiltin;

/// Run the `declare` builtin with the given arguments.
pub fn declare<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    BashBuiltin::find("declare")?.call(args)
}

/// Run the `local` builtin with the given arguments.
pub fn local<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    BashBuiltin::find("local")?.call(args)
}

/// Run the `set` builtin with the given arguments.
pub fn set<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    BashBuiltin::find("set")?.call(args)
}

/// Run the `shopt` builtin with the given arguments.
pub fn shopt<I>(args: I) -> crate::Result<ExecStatus>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    BashBuiltin::find("shopt")?.call(args)
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
