use std::collections::HashSet;
use std::ops::Deref;
use std::sync::{LazyLock, OnceLock};

use crate::report::Location;

static LANGUAGE: LazyLock<tree_sitter::Language> =
    LazyLock::new(|| tree_sitter_bash::LANGUAGE.into());

/// Wrapper for bash parse tree.
pub(crate) struct Tree<'a> {
    data: &'a [u8],
    tree: OnceLock<tree_sitter::Tree>,
}

impl<'a> Tree<'a> {
    /// Lazily parse the given data into a bash parse tree.
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, tree: Default::default() }
    }

    pub(crate) fn iter_global_nodes(&self) -> impl Iterator<Item = Node> {
        IterNodes::new(self.data, self.tree().walk(), &["function_definition"])
    }

    fn tree(&self) -> &tree_sitter::Tree {
        self.tree.get_or_init(|| {
            // TODO: Re-use parser instead of recreating it per pkg, this is currently difficult
            // because parser.parse() requires a mutable Parser reference.
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
            let iter = IterNodes::new(self.data, cursor, &[]);
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

/// Wrapper for bash parse tree node.
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

impl From<&Node<'_>> for Location {
    fn from(value: &Node<'_>) -> Self {
        Self {
            line: value.inner.start_position().row + 1,
            column: value.inner.start_position().column + 1,
        }
    }
}

/// Iterable for a bash parse tree using a given tree walking cursor.
struct IterNodes<'a> {
    data: &'a [u8],
    cursor: tree_sitter::TreeCursor<'a>,
    skip: HashSet<String>,
    seen: HashSet<usize>,
}

impl<'a> IterNodes<'a> {
    fn new(data: &'a [u8], cursor: tree_sitter::TreeCursor<'a>, skip: &[&str]) -> Self {
        Self {
            data,
            cursor,
            skip: skip.iter().map(|s| s.to_string()).collect(),
            seen: Default::default(),
        }
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
