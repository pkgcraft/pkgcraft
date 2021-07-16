use pkgcraft;

pub fn main() -> Result<(), &'static str> {
    pkgcraft::lib_init()?;
    Ok(())
}
