use strum::{AsRefStr, EnumIter, EnumString};

use crate::report::Report;

#[derive(AsRefStr, EnumIter, EnumString, Debug, PartialEq, Eq, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum Reporter {
    Simple(SimpleReporter),
}

impl Reporter {
    pub fn report(&self, report: &Report) {
        match self {
            Self::Simple(r) => r.report(report),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct SimpleReporter {}

impl SimpleReporter {
    pub fn report(&self, report: &Report) {
        println!("{report}");
    }
}
