use std::borrow::Borrow;

pub trait FilterLines {
    fn filter_lines(&self) -> Box<dyn Iterator<Item = (usize, &str)> + '_>;
}

impl<T: Borrow<str>> FilterLines for T {
    fn filter_lines(&self) -> Box<dyn Iterator<Item = (usize, &str)> + '_> {
        let iter = self
            .borrow()
            .lines()
            .map(|s| s.trim())
            .enumerate()
            .map(|(i, s)| (i + 1, s))
            .filter(|(_, s)| !s.is_empty() && !s.starts_with('#'));

        Box::new(iter)
    }
}
