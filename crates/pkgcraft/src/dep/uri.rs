use std::fmt;

use crate::Error;

/// Uri object.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uri {
    uri: String,
    rename: Option<String>,
}

impl Uri {
    /// Try to create a new Uri.
    pub(crate) fn try_new(uri: &str, rename: Option<&str>) -> crate::Result<Self> {
        let uri = Self {
            uri: uri.trim().to_string(),
            rename: rename.map(Into::into),
        };

        // rudimentary URI validity check since full parsing isn't used
        if uri.filename().is_empty() {
            return Err(Error::InvalidValue(format!("URI missing filename: {uri}")));
        }

        Ok(uri)
    }

    /// Return the file name for the Uri.
    pub fn filename(&self) -> &str {
        self.rename.as_deref().unwrap_or_else(|| {
            self.uri
                .rsplit_once('/')
                .map(|(_, s)| s)
                .unwrap_or(&self.uri)
        })
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.uri)?;
        if let Some(value) = &self.rename {
            write!(f, " -> {value}")?;
        }
        Ok(())
    }
}

impl AsRef<str> for Uri {
    fn as_ref(&self) -> &str {
        &self.uri
    }
}
