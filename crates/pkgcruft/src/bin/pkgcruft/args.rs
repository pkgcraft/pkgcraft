use std::io::{stdin, IsTerminal};

pub(super) struct StdinArgs<'a> {
    iter: Box<dyn Iterator<Item = String> + 'a>,
}

impl<'a> StdinArgs<'a> {
    /// Split arguments into separate strings by whitespace.
    pub(super) fn split_whitespace(self) -> StdinArgs<'a> {
        let iter = self.iter.flat_map(|s| {
            s.split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        });

        StdinArgs { iter: Box::new(iter) }
    }
}

impl Iterator for StdinArgs<'_> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Pull values from stdin if it's not a terminal and the first argument is `-`, otherwise use the
/// provided arguments.
pub(super) trait StdinOrArgs {
    fn stdin_or_args<'a>(self) -> StdinArgs<'a>
    where
        Self: IntoIterator<Item = String> + Sized + 'a,
    {
        let mut iter = self.into_iter().peekable();
        let iter: Box<dyn Iterator<Item = String>> = match iter.peek().map(|s| s.as_str()) {
            Some("-") if !stdin().is_terminal() => Box::new(stdin().lines().map_while(Result::ok)),
            _ => Box::new(iter),
        };

        StdinArgs { iter }
    }
}

impl StdinOrArgs for Vec<String> {}
