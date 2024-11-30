use std::collections::HashSet;
use std::fmt;
use std::ops::Deref;
use std::sync::{LazyLock, OnceLock};

use crate::report::Location;

static LANGUAGE: LazyLock<tree_sitter::Language> =
    LazyLock::new(|| tree_sitter_bash::LANGUAGE.into());

/// Wrapper for a lazily parsed bash tree.
#[derive(Debug, Clone)]
pub(crate) struct Tree<'a> {
    data: &'a [u8],
    tree: OnceLock<tree_sitter::Tree>,
}

impl<'a> Tree<'a> {
    /// Create a new bash parse tree from the given data.
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, tree: Default::default() }
    }

    /// Return an iterator over all global nodes, skipping function scope.
    pub(crate) fn iter_global(&self) -> impl Iterator<Item = Node> {
        self.into_iter().skip(["function_definition"])
    }

    /// Return an iterator over all function nodes, skipping global scope.
    pub(crate) fn iter_func(&self) -> impl Iterator<Item = Node> {
        self.into_iter()
            .filter(|x| x.kind() == "function_definition")
            .flatten()
    }

    /// Return the parse tree.
    fn tree(&self) -> &tree_sitter::Tree {
        self.tree.get_or_init(|| {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&LANGUAGE)
                .expect("failed loading bash grammar");
            parser.parse(self.data, None).expect("failed parsing bash")
        })
    }

    /// Return the last node for a given position if one exists.
    pub(crate) fn last_node_for_position(&self, row: usize, column: usize) -> Option<Node> {
        let mut cursor = self.tree().walk();
        let point = tree_sitter::Point::new(row, column);
        cursor.goto_first_child_for_point(point).map(|_| {
            let mut prev_node = cursor.node();
            let iter = IterNodes::new(self.data, cursor);
            for node in iter {
                if node.start_position().row > row {
                    break;
                }
                prev_node = node.inner;
            }
            Node {
                inner: prev_node,
                data: self.data,
            }
        })
    }
}

impl Deref for Tree<'_> {
    type Target = tree_sitter::Tree;

    fn deref(&self) -> &Self::Target {
        self.tree()
    }
}

impl<'a> IntoIterator for &'a Tree<'a> {
    type Item = Node<'a>;
    type IntoIter = IterNodes<'a>;

    fn into_iter(self) -> Self::IntoIter {
        IterNodes::new(self.data, self.tree().walk())
    }
}

/// Wrapper for bash parse tree node.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Node<'a> {
    inner: tree_sitter::Node<'a>,
    data: &'a [u8],
}

impl Node<'_> {
    /// Get the string value of a given node.
    pub(crate) fn as_str(&self) -> &str {
        self.inner.utf8_text(self.data).unwrap()
    }

    /// Get the name of a given node if it exists.
    pub(crate) fn name(&self) -> Option<&str> {
        self.inner
            .child_by_field_name("name")
            .map(|x| x.utf8_text(self.data).unwrap())
    }

    /// Return the node's line number.
    pub(crate) fn line(&self) -> usize {
        self.inner.start_position().row + 1
    }
}

impl<'a> Deref for Node<'a> {
    type Target = tree_sitter::Node<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl fmt::Display for Node<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl<'a> IntoIterator for Node<'a> {
    type Item = Node<'a>;
    type IntoIter = IterNodes<'a>;

    fn into_iter(self) -> Self::IntoIter {
        IterNodes::new(self.data, self.walk())
    }
}

impl From<&Node<'_>> for Location {
    fn from(value: &Node<'_>) -> Self {
        Self {
            line: value.inner.start_position().row + 1,
            column: value.inner.start_position().column + 1,
        }
    }
}

/// Iterable for a bash parse tree using a given tree walking cursor.
pub(crate) struct IterNodes<'a> {
    data: &'a [u8],
    cursor: tree_sitter::TreeCursor<'a>,
    skip: HashSet<String>,
    seen: HashSet<usize>,
}

impl<'a> IterNodes<'a> {
    fn new(data: &'a [u8], cursor: tree_sitter::TreeCursor<'a>) -> Self {
        Self {
            data,
            cursor,
            skip: Default::default(),
            seen: Default::default(),
        }
    }

    fn skip<I>(mut self, kinds: I) -> Self
    where
        I: IntoIterator,
        I::Item: std::fmt::Display,
    {
        self.skip = kinds.into_iter().map(|s| s.to_string()).collect();
        self
    }
}

impl<'a> Iterator for IterNodes<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let node = self.cursor.node();
            if (!self.seen.contains(&node.id())
                && !self.skip.contains(node.kind())
                && self.cursor.goto_first_child())
                || self.cursor.goto_next_sibling()
            {
                return Some(Node {
                    inner: self.cursor.node(),
                    data: self.data,
                });
            } else if self.cursor.goto_parent() {
                self.seen.insert(self.cursor.node().id());
            } else {
                return None;
            }
        }
    }
}
