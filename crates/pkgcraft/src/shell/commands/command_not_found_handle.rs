use scallop::builtins::shell_builtins;
use scallop::{Error, ExecStatus};

use super::make_builtin;

static LONG_DOC: &str = "\
Executed when the search for a command is unsuccessful.

This handles PATH search failures instead of using the command_not_found_handle() function method
instead.
";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let Some(cmd) = args.first().copied() else {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    };

    let (_, disabled) = shell_builtins();
    let error = if disabled.contains(cmd) {
        "disabled builtin"
    } else {
        "unknown command"
    };

    Err(Error::Base(format!("{error}: {cmd}")))
}

const USAGE: &str = "for internal use only";
make_builtin!("command_not_found_handle", command_not_found_handle_builtin);

#[cfg(test)]
mod tests {
    use scallop::source;
    use scallop::variables::{bind, optional};

    use crate::test::assert_err_re;

    #[test]
    fn fatal() {
        bind("VAR", "1", None, None).unwrap();

        let r = source::string("nonexistent && VAR=2");
        assert_err_re!(r, "^line 1: unknown command: nonexistent$");

        // verify bash state
        assert_eq!(optional("VAR").unwrap(), "1");

        // verify builtin overrides the function equivalent
        source::string("command_not_found_handle() { VAR=2; }").unwrap();
        let r = source::string("nonexistent && VAR=2");
        assert_err_re!(r, "^line 1: unknown command: nonexistent$");

        // verify bash state
        assert_eq!(optional("VAR").unwrap(), "1");
    }

    #[test]
    fn nonfatal() {
        bind("VAR", "1", None, None).unwrap();
        source::string("nonfatal nonexistent; VAR=2").unwrap();
        assert_eq!(optional("VAR").unwrap(), "2");
    }
}
