use super::{has::run as has, make_builtin};

const LONG_DOC: &str = "Deprecated synonym for has.";
const USAGE: &str = "hasq needle ${haystack}";
make_builtin!("hasq", hasq_builtin, has, LONG_DOC, USAGE, BUILTIN);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);
}
