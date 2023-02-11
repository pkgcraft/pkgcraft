use std::str::FromStr;

use aho_corasick::AhoCorasick;
use strum::IntoEnumIterator;

pub trait EnumVariable: std::fmt::Display + FromStr + IntoEnumIterator {
    type Object;
    fn value(&self, obj: &Self::Object) -> String;
}

pub trait FormatString {
    type Object;
    type FormatKey: EnumVariable<Object = Self::Object>;

    fn format(&self, fmt: &str, obj: &Self::Object) -> String {
        let patterns: Vec<_> = Self::FormatKey::iter()
            .flat_map(|k| [format!("{{{k}}}"), format!("[{k}]")])
            .collect();
        let ac = AhoCorasick::new(patterns);
        let mut result = String::new();
        ac.replace_all_with(fmt, &mut result, |_mat, mat_str, dst| {
            // strip match wrappers and convert to Key variant
            let mat_type = &mat_str[0..1];
            let key_str = &mat_str[1..mat_str.len() - 1];
            let key = Self::FormatKey::from_str(key_str)
                .unwrap_or_else(|_| panic!("invalid pattern: {key_str}"));

            // replace match with the related value
            match key.value(obj).as_str() {
                "" if mat_type == "{" => dst.push_str("<unset>"),
                s => dst.push_str(s),
            }

            true
        });
        result
    }
}
