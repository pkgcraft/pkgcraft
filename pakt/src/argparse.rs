use anyhow::{anyhow, Context, Result};

// convert string to boolean value
pub fn str_to_bool(s: &str) -> Result<bool> {
    match s.to_lowercase().as_ref() {
        "y" | "yes" | "true" | "1" => Ok(true),
        "n" | "no" | "false" | "0" => Ok(false),
        _ => Err(anyhow!("not a boolean value: {:?}", s)),
    }
}

/// Verify a given value is a positive integer (u64).
pub fn positive_int(v: &str) -> Result<()> {
    let int = v
        .parse::<u64>()
        .context(format!("invalid positive integer: {:?}", v))?;
    if int < 1 {
        Err(anyhow!("must be >= 1"))
    } else {
        Ok(())
    }
}
