use std::path::Path;

use scallop::variables::{bind, string_value, string_vec};
use scallop::{source, Error, Result};

use super::BUILD_DATA;

pub fn source_ebuild<P: AsRef<Path>>(ebuild: P) -> Result<()> {
    let ebuild = ebuild.as_ref();
    if !ebuild.exists() {
        return Err(Error::new(format!("nonexistent ebuild: {:?}", ebuild)));
    }

    source::file(&ebuild).unwrap();

    // TODO: export default for $S

    BUILD_DATA.with(|d| {
        let mut d = d.borrow_mut();
        let eapi = d.eapi;

        // set RDEPEND=DEPEND if RDEPEND is unset
        if eapi.has("rdepend_default") && string_value("RDEPEND").is_none() {
            let depend = string_value("DEPEND").unwrap_or_else(|| String::from(""));
            bind("RDEPEND", &depend, None);
        }

        // prepend metadata keys that incrementally accumulate to eclass values
        for var in &eapi.incremental_keys {
            if let Some(data) = string_vec(var) {
                let deque = d.get_deque(var);
                // TODO: extend_left() should be implemented upstream for VecDeque
                for item in data.into_iter().rev() {
                    deque.push_front(item);
                }
            }
        }
    });

    Ok(())
}
