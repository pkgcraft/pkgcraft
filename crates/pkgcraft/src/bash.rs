use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::LazyLock;

use crate::shell::phase::PhaseKind;
use crate::shell::scope::Scope;

static CONDITIONALS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    ["test_command", "if_statement", "list"]
        .into_iter()
        .map(Into::into)
        .collect()
});

static LANGUAGE: LazyLock<tree_sitter::Language> =
    LazyLock::new(|| tree_sitter_bash::LANGUAGE.into());

/// Wrapper for a lazily parsed bash tree.
#[derive(Debug, Clone)]
pub struct Tree<'a> {
    data: &'a [u8],
    tree: tree_sitter::Tree,
}

impl<'a> Tree<'a> {
    /// Create a new bash parse tree from the given data.
    pub fn new(data: &'a [u8]) -> Self {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&LANGUAGE)
            .expect("failed loading bash grammar");
        let tree = parser.parse(data, None).expect("failed parsing bash");
        Self { data, tree }
    }

    /// Return an iterator over global nodes, skipping function scope.
    pub fn iter_global(&self) -> impl Iterator<Item = Node> {
        self.into_iter().skip(["function_definition"])
    }

    /// Return an iterator over function nodes, skipping global scope.
    pub fn iter_func(&self) -> impl Iterator<Item = Node> {
        self.into_iter()
            .filter(|x| x.kind() == "function_definition")
    }

    /// Return the last node for a given position if one exists.
    pub fn last_node_for_position(&self, row: usize, column: usize) -> Option<Node> {
        let mut cursor = self.tree.walk();
        let point = tree_sitter::Point::new(row, column);
        cursor.goto_first_child_for_point(point).map(|_| {
            let mut prev_node = cursor.node();
            let iter = IterRecursive::new(self.data, cursor);
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
        &self.tree
    }
}

impl<'a> IntoIterator for &'a Tree<'a> {
    type Item = Node<'a>;
    type IntoIter = IterRecursive<'a>;

    fn into_iter(self) -> Self::IntoIter {
        IterRecursive::new(self.data, self.tree.walk())
    }
}

/// Wrapper for bash parse tree node.
#[derive(Clone, Copy)]
pub struct Node<'a> {
    inner: tree_sitter::Node<'a>,
    data: &'a [u8],
}

impl<'a> Node<'a> {
    /// Get the string value of a given node.
    pub fn as_str(&self) -> &str {
        self.inner.utf8_text(self.data).unwrap()
    }

    /// Get the name of a given node if it exists.
    pub fn name(&self) -> Option<&str> {
        self.inner
            .child_by_field_name("name")
            .map(|x| x.utf8_text(self.data).unwrap())
    }

    /// Return the node's line number.
    pub fn line(&self) -> usize {
        self.inner.start_position().row + 1
    }

    /// Return the parent node if one exists.
    pub fn parent(&self) -> Option<Self> {
        self.inner
            .parent()
            .map(|inner| Self { inner, data: self.data })
    }

    /// Return true if the node is location inside a conditional statement, otherwise false.
    pub fn in_conditional(&self) -> bool {
        let mut node = *self;
        while let Some(x) = node.parent() {
            if CONDITIONALS.contains(x.kind()) {
                return true;
            }
            node = x;
        }
        false
    }

    /// Return the function name the node is in if it exists.
    pub fn in_function(&self) -> Option<String> {
        let mut node = *self;
        while let Some(x) = node.parent() {
            if node.kind() == "function_definition" {
                return node.name().map(Into::into);
            }
            node = x;
        }
        None
    }

    // TODO: handle nested functions
    /// Return the node's scope if it exists.
    pub fn in_scope(&self) -> Option<Scope> {
        match self.in_function() {
            None => Some(Scope::Global),
            Some(func) => func.parse::<PhaseKind>().ok().map(Into::into),
        }
    }

    /// Return this node's children.
    pub fn children<'cursor>(
        &'cursor self,
        cursor: &'cursor mut tree_sitter::TreeCursor<'a>,
    ) -> impl Iterator<Item = Node<'a>> + 'cursor {
        self.inner
            .children(cursor)
            .map(move |inner| Self { inner, data: self.data })
    }
}

impl PartialEq<tree_sitter::Node<'_>> for Node<'_> {
    fn eq(&self, other: &tree_sitter::Node) -> bool {
        &self.inner == other
    }
}

impl<'a> Deref for Node<'a> {
    type Target = tree_sitter::Node<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl PartialEq for Node<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl Eq for Node<'_> {}

impl Hash for Node<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl fmt::Display for Node<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl fmt::Debug for Node<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Node {{ kind: {}, value: {self} }}", self.kind())
    }
}

impl AsRef<str> for Node<'_> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<'a> IntoIterator for Node<'a> {
    type Item = Node<'a>;
    type IntoIter = IterRecursive<'a>;

    fn into_iter(self) -> Self::IntoIter {
        IterRecursive::new(self.data, self.walk())
    }
}

impl<'a> IntoIterator for &Node<'a> {
    type Item = Node<'a>;
    type IntoIter = IterRecursive<'a>;

    fn into_iter(self) -> Self::IntoIter {
        (*self).into_iter()
    }
}

/// Iterable for a bash parse tree using a given tree walking cursor.
pub struct IterRecursive<'a> {
    data: &'a [u8],
    cursor: tree_sitter::TreeCursor<'a>,
    skip: HashSet<String>,
    seen: HashSet<usize>,
}

impl<'a> IterRecursive<'a> {
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

impl<'a> Iterator for IterRecursive<'a> {
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
