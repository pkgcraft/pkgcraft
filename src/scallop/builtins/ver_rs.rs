use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::variables::string_value;
use scallop::{Error, Result};

use super::{parse, version_split};

static LONG_DOC: &str = "\
Perform string substitution on package version strings.

Returns -1 on error.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let pv = string_value("PV").unwrap_or_else(|| String::from(""));
    let pv = pv.as_str();
    let (ver, args) = match args.len() {
        n if n < 2 => return Err(Error::new(format!("requires 2 or more args, got {}", n))),

        // even number of args uses $PV
        n if n % 2 == 0 => (pv, args),

        // odd number of args uses the last arg as the version
        _ => {
            let idx = args.len() - 1;
            (args[idx], &args[..idx])
        }
    };

    // Split version string into separators and components, note that the version string doesn't
    // have to follow the spec since args like ".1.2.3" are allowed.
    let mut version_parts = version_split(ver);

    // iterate over (range, separator) pairs
    let mut args_iter = args.chunks_exact(2);
    while let Some(&[range, sep]) = args_iter.next() {
        let len = version_parts.len();
        let (start, end) = parse::range(range, len / 2)?;
        (start..=end)
            .map(|i| i * 2)
            .take_while(|i| i < &len)
            .for_each(|i| {
                if (i > 0 && i < len - 1) || !version_parts[i].is_empty() {
                    version_parts[i] = sep;
                }
            });
    }

    println!("{}", version_parts.join(""));

    Ok(ExecStatus::Success)
}

pub static BUILTIN: Builtin = Builtin {
    name: "ver_rs",
    func: run,
    help: LONG_DOC,
    usage: "ver_rs 2 - 1.2.3",
    error_func: Some(output_error_func),
};
