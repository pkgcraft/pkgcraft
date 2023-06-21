use std::env;

mod dep;
mod pkg;
mod predicates;
mod repo;
mod version;

#[ctor::ctor]
fn initialize() {
    env::set_var("PKGCRAFT_NO_CONFIG", "true");
}
