use std::ffi::{c_char, CStr, CString};
use std::ptr;

use crate::bash;

pub struct Words {
    words: *mut bash::WordList,
    owned: bool,
}

impl TryFrom<Words> for Vec<String> {
    type Error = std::str::Utf8Error;

    fn try_from(words: Words) -> Result<Self, Self::Error> {
        words
            .into_iter()
            .map(|r| r.map(|s| s.to_string()))
            .collect()
    }
}

impl Drop for Words {
    fn drop(&mut self) {
        if self.owned {
            unsafe { bash::dispose_words(self.words) };
        }
    }
}

impl<'a> IntoIterator for &'a Words {
    type Item = Result<&'a str, std::str::Utf8Error>;
    type IntoIter = WordsIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        WordsIter {
            words: unsafe { self.words.as_ref() },
        }
    }
}

pub struct WordsIter<'a> {
    words: Option<&'a bash::WordList>,
}

impl<'a> Iterator for WordsIter<'a> {
    type Item = Result<&'a str, std::str::Utf8Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.words.map(|w| unsafe {
            self.words = w.next.as_ref();
            let word = (*w.word).word;
            CStr::from_ptr(word).to_str()
        })
    }
}

/// Support conversion from a given object into a [`Words`].
pub trait IntoWords {
    /// Convert an owned bash word list into a [`Words`] that frees the word list on drop.
    fn into_words(self) -> Words;
    /// Convert a borrowed bash word list into a [`Words`].
    fn to_words(self) -> Words;
}

impl IntoWords for *mut bash::WordList {
    fn into_words(self) -> Words {
        Words { words: self, owned: true }
    }
    fn to_words(self) -> Words {
        Words { words: self, owned: false }
    }
}

impl<S: AsRef<str>> FromIterator<S> for Words {
    fn from_iter<I: IntoIterator<Item = S>>(iter: I) -> Self {
        let strs: Vec<_> = iter
            .into_iter()
            .map(|s| CString::new(s.as_ref()).unwrap())
            .collect();
        let mut ptrs: Vec<_> = strs.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        ptrs.push(ptr::null_mut());
        let words = unsafe { bash::strvec_to_word_list(ptrs.as_mut_ptr(), 1, 0) };
        Words { words, owned: true }
    }
}

impl From<&Words> for *mut bash::WordList {
    fn from(val: &Words) -> Self {
        val.words
    }
}
