use std::str::FromStr;

use aho_corasick::AhoCorasick;
use strum::IntoEnumIterator;

pub(crate) trait EnumVariable<'a>:
    std::fmt::Display + FromStr + IntoEnumIterator
{
    type Object;
    fn value(&self, obj: &'a Self::Object) -> String;
}

pub(crate) trait FormatString<'a> {
    type Object;
    type FormatKey: EnumVariable<'a, Object = Self::Object>;

    fn format_str(&self, fmt: &'a str, obj: &'a Self::Object) -> anyhow::Result<String> {
        let patterns: Vec<_> = Self::FormatKey::iter()
            .flat_map(|k| [format!("{{{k}}}"), format!("[{k}]")])
            .collect();
        let ac = AhoCorasick::new(patterns)?;
        let mut result = String::new();
        ac.try_replace_all_with(fmt, &mut result, |_mat, mat_str, dst| {
            // strip match wrappers and convert to Key variant
            let mat_type = &mat_str[0..1];
            let key_str = &mat_str[1..mat_str.len() - 1];
            let key: Self::FormatKey = key_str
                .parse()
                .unwrap_or_else(|_| panic!("invalid pattern: {key_str}"));

            // replace match with the related value
            match key.value(obj).as_str() {
                "" if mat_type == "{" => dst.push_str("<unset>"),
                s => dst.push_str(s),
            }

            true
        })?;
        Ok(result)
    }
}
