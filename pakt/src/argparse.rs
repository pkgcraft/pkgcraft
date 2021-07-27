use std::str::FromStr;

#[derive(Debug)]
pub struct BoolArg {
    value: bool,
}

impl FromStr for BoolArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl BoolArg {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_ref() {
            "y" | "yes" | "true" | "1" => Ok(BoolArg { value: true }),
            "n" | "no" | "false" | "0" => Ok(BoolArg { value: false }),
            _ => Err(format!("not a boolean value: {:?}", s)),
        }
    }

    pub fn is_true(&self) -> bool {
        self.value
    }
}
