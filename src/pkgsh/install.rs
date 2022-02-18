use std::path::Path;

use scallop::Result;

pub(crate) fn create_link<P: AsRef<Path>>(_hard: bool, _source: P, _target: P) -> Result<()> {
    // TODO: fill out this stub
    Ok(())
}
