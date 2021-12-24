use once_cell::sync::Lazy;
use regex::Regex;

static VERSION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?P<sep>[[:^alnum:]]+)?(?P<comp>[[:digit:]]+|[[:alpha:]]+)?").unwrap()
});

/// Split version string into a vector of separators and components.
pub fn version_split(ver: &str) -> Vec<&str> {
    let mut version_parts = Vec::new();
    for caps in VERSION_RE.captures_iter(ver) {
        let sep = caps.name("sep").map_or("", |m| m.as_str());
        let comp = caps.name("comp").map_or("", |m| m.as_str());
        version_parts.extend([sep, comp]);
    }
    version_parts
}

peg::parser! {
    pub grammar cmd() for str {
        // Parse ranges used with the ver_rs and ver_cut commands.
        pub rule range(max: usize) -> (usize, usize)
            = start_s:$(['0'..='9']+) "-" end_s:$(['0'..='9']+) {
                let start = start_s.parse::<usize>().unwrap();
                let end = end_s.parse::<usize>().unwrap();
                (start, end)
            } / start_s:$(['0'..='9']+) "-" {
                match start_s.parse::<usize>().unwrap() {
                    start if start <= max => (start, max),
                    start => (start, start),
                }
            } / start_s:$(['0'..='9']+) {
                let start = start_s.parse::<usize>().unwrap();
                (start, start)
            }
    }
}

// provide public parsing functionality while converting error types
pub mod parse {
    use crate::peg::peg_error;

    use super::cmd;

    #[inline]
    pub fn range(s: &str, max: usize) -> crate::Result<(usize, usize)> {
        let (start, end) =
            cmd::range(s, max).map_err(|e| peg_error(format!("invalid range: {:?}", s), s, e))?;
        if end < start {
            return Err(crate::Error::InvalidValue(format!(
                "start of range ({}) is greater than end ({})",
                start, end
            )));
        }
        Ok((start, end))
    }
}
