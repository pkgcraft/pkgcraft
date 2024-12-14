use std::borrow::Cow;
use std::fmt;

use crate::repo::ebuild::EbuildRepo;
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

    /// Return the string slice for the [`Uri`].
    pub fn as_str(&self) -> &str {
        &self.uri
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

    /// Generate a fetchable URI, replacing existing mirrors.
    pub(crate) fn fetchable(&self, repo: &EbuildRepo) -> crate::Result<Cow<Self>> {
        if let Some((name, suffix)) = self
            .as_ref()
            .strip_prefix("mirror://")
            .and_then(|x| x.split_once('/'))
        {
            // TODO: support some type of mirror choice algorithm
            if let Some(prefix) = repo.mirrors().get(name).and_then(|s| s.first()) {
                let mut uri = self.clone();
                uri.uri = format!("{prefix}/{suffix}");
                Ok(Cow::Owned(uri))
            } else {
                Err(Error::InvalidValue(format!("unknown mirror: {name}: {self}")))
            }
        } else {
            Ok(Cow::Borrowed(self))
        }
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
