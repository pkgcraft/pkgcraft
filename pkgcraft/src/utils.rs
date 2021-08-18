// Return a string slice stripping the given character from the right side. Note that this assumes
// the string only contains ASCII characters.
pub(crate) fn rstrip(s: &str, c: char) -> &str {
    let mut count = 0;
    for x in s.chars().rev() {
        if x != c {
            break;
        }
        count += 1;
    }
    // We can't use chars.as_str().len() since std::iter::Rev doesn't support it.
    &s[..s.len() - count]
}
