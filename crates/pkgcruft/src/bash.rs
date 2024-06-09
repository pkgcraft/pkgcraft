use std::collections::HashSet;
use std::fmt;
use std::ops::Deref;
use std::sync::OnceLock;

pub(crate) struct Tree<'a> {
    data: &'a [u8],
    tree: OnceLock<tree_sitter::Tree>,
}

impl<'a> Tree<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, tree: OnceLock::new() }
    }

    pub(crate) fn iter_global_nodes(&self) -> IterNodes {
        IterNodes::new(self.data, self.tree().root_node(), ["function_definition"])
    }

    fn tree(&self) -> &tree_sitter::Tree {
        self.tree.get_or_init(|| {
            // TODO: Re-use parser instead of recreating it per pkg, this is currently difficult
            // because parser.parse() requires a mutable Parser reference.
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_bash::language())
                .expect("failed loading bash grammar");
            parser.parse(self.data, None).expect("failed parsing bash")
        })
    }
}

impl Deref for Tree<'_> {
    type Target = tree_sitter::Tree;

    fn deref(&self) -> &Self::Target {
        self.tree()
    }
}

pub(crate) struct Node<'a> {
    node: tree_sitter::Node<'a>,
    data: &'a [u8],
}

impl<'a> Node<'a> {
    fn new(node: tree_sitter::Node<'a>, data: &'a [u8]) -> Self {
        Self { data, node }
    }

    /// Get the string value of a given node.
    pub(crate) fn as_str(&self) -> &str {
        self.node.utf8_text(self.data).unwrap()
    }

    /// Get the name of a given node if it exists.
    pub(crate) fn name(&self) -> Option<&str> {
        self.node
            .child_by_field_name("name")
            .map(|x| x.utf8_text(self.data).unwrap())
    }

    /// Return the node's line number.
    pub(crate) fn line(&self) -> usize {
        self.node.start_position().row + 1
    }
}

impl<'a> Deref for Node<'a> {
    type Target = tree_sitter::Node<'a>;

    fn deref(&self) -> &Self::Target {
        &self.node
    }
}

pub(crate) struct IterNodes<'a> {
    data: &'a [u8],
    cursor: tree_sitter::TreeCursor<'a>,
    skip: HashSet<String>,
    seen: HashSet<usize>,
}

impl<'a> IterNodes<'a> {
    fn new<I, S>(data: &'a [u8], node: tree_sitter::Node<'a>, skip: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: fmt::Display,
    {
        Self {
            data,
            cursor: node.walk(),
            skip: skip.into_iter().map(|s| s.to_string()).collect(),
            seen: Default::default(),
        }
    }
}

impl<'a> Iterator for IterNodes<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.cursor.node();
        if (!self.seen.contains(&node.id())
            && !self.skip.contains(node.kind())
            && self.cursor.goto_first_child())
            || self.cursor.goto_next_sibling()
        {
            let node = self.cursor.node();
            Some(Node::new(node, self.data))
        } else if self.cursor.goto_parent() {
            self.seen.insert(self.cursor.node().id());
            self.next()
        } else {
            None
        }
    }
}
