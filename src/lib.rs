pub mod atom;
mod config;
pub mod depspec;
pub mod eapi;
mod macros;
mod repo;
mod utils;

pub fn lib_init() -> Result<(), &'static str> {
    println!("using pkgcraft");
    Ok(())
}
