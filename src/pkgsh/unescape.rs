pub(crate) fn unescape_string(s: &str) -> Result<String, Error> {
    UnescapeString::new(s).collect()
}

pub(crate) struct UnescapeString<'a> {
    s: std::str::Chars<'a>,
}

impl<'a> UnescapeString<'a> {
    pub(crate) fn new(s: &'a str) -> Self {
        UnescapeString { s: s.chars() }
    }
}

impl Iterator for UnescapeString<'_> {
    type Item = Result<char, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.s.next().map(|c| match c {
            '\\' => match self.s.next() {
                None => Err(Error::EscapeAtEndOfString),
                Some('n') => Ok('\n'),
                Some('t') => Ok('\t'),
                Some('\\') => Ok('\\'),
                Some(c) => Err(Error::UnrecognizedEscape(format!(r"\{}", c))),
            },
            c => Ok(c),
        })
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("escape characater at the end of string")]
    EscapeAtEndOfString,
    #[error("unrecognized escape: {0}")]
    UnrecognizedEscape(String),
}

impl From<Error> for scallop::Error {
    fn from(e: Error) -> Self {
        scallop::Error::Base(e.to_string())
    }
}
