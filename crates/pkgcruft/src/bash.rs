use std::collections::HashSet;
use std::fmt;
use std::ops::Deref;

pub(crate) struct Tree<'a> {
    data: &'a [u8],
    pub(crate) tree: tree_sitter::Tree,
}

impl<'a> Tree<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        // TODO: Re-use parser instead of recreating it per pkg, this is currently difficult
        // because parser.parse() requires a mutable Parser reference.
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_bash::language())
            .expect("failed loading bash grammar");
        let tree = parser.parse(data, None).expect("failed parsing bash");
        Self { data, tree }
    }

    pub(crate) fn iter_global_nodes(&self) -> IterNodes {
        IterNodes::new(self.tree.root_node(), ["function_definition"])
    }

    pub(crate) fn node_name(&self, node: tree_sitter::Node) -> &str {
        let node = node.child_by_field_name("name").unwrap();
        self.node_str(node)
    }

    pub(crate) fn node_str(&self, node: tree_sitter::Node) -> &str {
        node.utf8_text(self.data).unwrap()
    }
}

impl Deref for Tree<'_> {
    type Target = tree_sitter::Tree;

    fn deref(&self) -> &Self::Target {
        &self.tree
    }
}

pub(crate) struct IterNodes<'a> {
    cursor: tree_sitter::TreeCursor<'a>,
    skip: HashSet<String>,
    seen: HashSet<usize>,
}

impl<'a> IterNodes<'a> {
    fn new<I, S>(node: tree_sitter::Node<'a>, skip: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: fmt::Display,
    {
        Self {
            cursor: node.walk(),
            skip: skip.into_iter().map(|s| s.to_string()).collect(),
            seen: Default::default(),
        }
    }
}

impl<'a> Iterator for IterNodes<'a> {
    type Item = tree_sitter::Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.cursor.node();
        if (!self.seen.contains(&node.id())
            && !self.skip.contains(node.kind())
            && self.cursor.goto_first_child())
            || self.cursor.goto_next_sibling()
        {
            Some(self.cursor.node())
        } else if self.cursor.goto_parent() {
            self.seen.insert(self.cursor.node().id());
            self.next()
        } else {
            None
        }
    }
}
