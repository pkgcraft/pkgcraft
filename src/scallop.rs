use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};

use indexmap::IndexSet;

use crate::eapi::Eapi;
use crate::Error;

pub mod builtins;
pub mod functions;

#[derive(Debug, Default, Clone)]
pub struct BuildData {
    pub repo: String,
    pub eapi: &'static Eapi,

    pub desttree: String,
    pub insdesttree: String,
    pub iuse_effective: HashSet<String>,
    pub r#use: HashSet<String>,

    /// Eclasses directly inherited by an ebuild.
    pub inherit: Vec<String>,
    /// Full set of eclasses inherited by an ebuild.
    pub inherited: IndexSet<String>,

    // ebuild metadata fields
    pub iuse: VecDeque<String>,
    pub required_use: VecDeque<String>,
    pub depend: VecDeque<String>,
    pub rdepend: VecDeque<String>,
    pub pdepend: VecDeque<String>,
    pub bdepend: VecDeque<String>,
    pub idepend: VecDeque<String>,
    pub properties: VecDeque<String>,
    pub restrict: VecDeque<String>,
}

impl BuildData {
    pub fn get_deque(&mut self, name: &str) -> &mut VecDeque<String> {
        match name {
            "IUSE" => &mut self.iuse,
            "REQUIRED_USE" => &mut self.required_use,
            "DEPEND" => &mut self.depend,
            "RDEPEND" => &mut self.rdepend,
            "PDEPEND" => &mut self.pdepend,
            "BDEPEND" => &mut self.bdepend,
            "IDEPEND" => &mut self.idepend,
            "PROPERTIES" => &mut self.properties,
            "RESTRICT" => &mut self.restrict,
            s => panic!("unknown field name: {}", s),
        }
    }
}

thread_local! {
    pub static BUILD_DATA: RefCell<BuildData> = RefCell::new(Default::default());
}

impl From<Error> for scallop::Error {
    fn from(e: Error) -> Self {
        scallop::Error::new(e.to_string())
    }
}
