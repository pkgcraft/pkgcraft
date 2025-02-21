use std::borrow::Cow;

/// Unescape a given string.
pub(crate) fn unescape(s: &str) -> Result<Cow<str>, Error> {
    UnescapeString::unescape(s)
}

#[derive(Debug, Clone)]
pub(crate) struct UnescapeString<'a> {
    s: std::str::Chars<'a>,
    mutated: bool,
}

impl UnescapeString<'_> {
    pub(crate) fn unescape(s: &str) -> Result<Cow<str>, Error> {
        let unescape = UnescapeString {
            s: s.chars(),
            mutated: s.is_empty(),
        };
        if unescape.mutated()? {
            let s = unescape.collect::<Result<String, Error>>()?;
            Ok(Cow::Owned(s))
        } else {
            Ok(Cow::Borrowed(s))
        }
    }

    fn mutated(&self) -> Result<bool, Error> {
        let mut iter = self.clone();
        while iter.next().is_some() {}
        Ok(iter.mutated)
    }
}

impl Iterator for UnescapeString<'_> {
    type Item = Result<char, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.s.next().map(|c| match c {
            '\\' => match self.s.next() {
                Some('n') => {
                    self.mutated = true;
                    Ok('\n')
                }
                Some('t') => {
                    self.mutated = true;
                    Ok('\t')
                }
                Some('\\') => {
                    self.mutated = true;
                    Ok('\\')
                }
                Some(c) => Err(Error::UnrecognizedEscape(format!(r"\{c}"))),
                None => Err(Error::EscapeAtEndOfString),
            },
            c => Ok(c),
        })
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("escape character at the end of string")]
    EscapeAtEndOfString,
    #[error("unrecognized escape: {0}")]
    UnrecognizedEscape(String),
}

impl From<Error> for scallop::Error {
    fn from(e: Error) -> Self {
        scallop::Error::Base(e.to_string())
    }
}
