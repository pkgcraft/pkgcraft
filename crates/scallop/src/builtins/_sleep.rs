use std::thread;

use crate::builtins::make_builtin;
use crate::{Error, ExecStatus};

static LONG_DOC: &str = "Sleep for a given amount of time.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> crate::Result<ExecStatus> {
    let [value] = args else {
        return Err(Error::Base("requires 1 arg, got 0".into()));
    };

    let duration: humantime::Duration = value
        .parse()
        .map_err(|_| Error::Base(format!("invalid duration: {value}")))?;

    thread::sleep(*duration);

    Ok(ExecStatus::Success)
}

make_builtin!("sleep", sleep_builtin, run, LONG_DOC, "sleep 50ms");

#[cfg(test)]
mod tests {
    use crate::builtins::{self, sleep};
    use crate::source;
    use crate::test::assert_err_re;

    #[test]
    fn builtin() {
        // register and enable builtin
        builtins::register([sleep]);
        builtins::enable([sleep]).unwrap();

        // no args
        assert!(sleep.call(&[]).is_err());

        // missing unit
        let r = sleep.call(&["1"]);
        assert_err_re!(r, "^invalid duration: 1");
        // invalid value
        let r = sleep.call(&["abc"]);
        assert_err_re!(r, "^invalid duration: abc");
        // float values aren't supported
        let r = sleep.call(&["1.2ns"]);
        assert_err_re!(r, "^invalid duration: 1.2ns");

        // verify basic command directly from bash
        assert!(source::string("sleep 10ms").is_ok());
        assert!(source::string("sleep 1ns").is_ok());
    }
}
