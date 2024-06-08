use std::collections::HashSet;
use std::fmt;
use std::ops::Deref;

use pkgcraft::pkg::ebuild;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Restrict;
use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

/// All check runner source variants.
#[derive(
    AsRefStr,
    Display,
    EnumIter,
    EnumString,
    VariantNames,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Copy,
    Clone,
)]
#[strum(serialize_all = "kebab-case")]
pub enum SourceKind {
    Ebuild,
    EbuildRaw,
    EbuildParsed,
}

pub(crate) trait IterRestrict {
    type Item;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> impl Iterator<Item = Self::Item>;
}

pub(crate) struct Ebuild {
    pub(crate) repo: &'static Repo,
}

impl IterRestrict for Ebuild {
    type Item = ebuild::Pkg<'static>;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> impl Iterator<Item = Self::Item> {
        self.repo.iter_restrict(val)
    }
}

pub(crate) struct EbuildRaw {
    pub(crate) repo: &'static Repo,
}

impl IterRestrict for EbuildRaw {
    type Item = ebuild::raw::Pkg<'static>;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> impl Iterator<Item = Self::Item> {
        self.repo.iter_raw_restrict(val)
    }
}

pub(crate) struct PkgParsed {
    pkg: ebuild::raw::Pkg<'static>,
    pub(crate) tree: tree_sitter::Tree,
}

impl PkgParsed {
    pub(crate) fn iter_global_nodes(&self) -> IterNodes {
        IterNodes::new(self.tree.root_node(), ["function_definition"])
    }

    pub(crate) fn node_name(&self, node: tree_sitter::Node) -> &str {
        let node = node.child_by_field_name("name").unwrap();
        self.node_str(node)
    }

    pub(crate) fn node_str(&self, node: tree_sitter::Node) -> &str {
        node.utf8_text(self.data().as_bytes()).unwrap()
    }
}

impl Deref for PkgParsed {
    type Target = ebuild::raw::Pkg<'static>;

    fn deref(&self) -> &Self::Target {
        &self.pkg
    }
}

impl fmt::Display for PkgParsed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.pkg)
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

pub(crate) struct EbuildParsed {
    source: EbuildRaw,
}

impl EbuildParsed {
    pub(crate) fn new(repo: &'static Repo) -> Self {
        Self { source: EbuildRaw { repo } }
    }
}

impl IterRestrict for EbuildParsed {
    type Item = PkgParsed;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> impl Iterator<Item = Self::Item> {
        self.source.iter_restrict(val).map(|pkg| {
            // TODO: Re-use parser instead of recreating it per pkg, this is currently difficult
            // because parser.parse() requires a mutable Parser reference.
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_bash::language())
                .expect("failed loading bash grammar");
            let tree = parser.parse(pkg.data(), None).expect("failed parsing bash");
            PkgParsed { pkg, tree }
        })
    }
}
