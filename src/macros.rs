// convert &str to Option<String>
#[allow(unused_macros)] // currently only used in tests
macro_rules! opt_str {
    ($x:expr) => {
        Some($x.to_string())
    };
}
#[allow(unused_imports)] // currently only used in tests
pub(crate) use opt_str;

// convert Vec<&str> to Vec<String>
macro_rules! vec_str {
    ($x:expr) => {
        $x.iter().map(|&s| s.to_string()).collect()
    };
}
pub(crate) use vec_str;
