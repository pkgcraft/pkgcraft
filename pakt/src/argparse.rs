use anyhow::{anyhow, Result};

// convert string to boolean value
pub fn str_to_bool(s: &str) -> Result<bool> {
    match s.to_lowercase().as_ref() {
        "y" | "yes" | "true" | "1" => Ok(true),
        "n" | "no" | "false" | "0" => Ok(false),
        _ => Err(anyhow!("not a boolean value: {:?}", s)),
    }
}
