use std::fmt;

#[derive(Debug, PartialEq)]
pub struct Repo {
    pub id: String,
    pub path: String,
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.id, self.path)
    }
}
