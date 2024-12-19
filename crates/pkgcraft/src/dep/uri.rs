use std::fmt;

/// Uri object.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uri {
    uri: String,
    rename: Option<String>,
}

impl Uri {
    /// Create a new Uri.
    pub(crate) fn new(uri: &str, rename: Option<&str>) -> Self {
        // TODO: Verify URLs or fetch restricted file names once parsing is reworked to
        // allow custom errors.
        Self {
            uri: uri.trim().to_string(),
            rename: rename.map(Into::into),
        }
    }

    /// Return the string slice for the [`Uri`].
    pub fn as_str(&self) -> &str {
        &self.uri
    }

    /// Return the renamed file name for the Uri, if it exists.
    pub fn rename(&self) -> Option<&str> {
        self.rename.as_deref()
    }

    /// Return the file name for the Uri.
    pub fn filename(&self) -> &str {
        self.rename().unwrap_or_else(|| {
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
