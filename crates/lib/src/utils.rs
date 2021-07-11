// Return a string slice stripping the given character from the right side. Note that this assumes
// the string only contains ASCII characters.
pub fn rstrip(s: &str, c: char) -> &str {
    let mut chars = s.chars().rev();
    let mut count = 0;
    while let Some(x) = chars.next() {
        if x != c {
            break;
        }
        count += 1;
    }
    // We can't use chars.as_str().len() since std::iter::Rev doesn't support it.
    &s[..s.len() - count]
}
