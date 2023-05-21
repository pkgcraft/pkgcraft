use std::ffi::CStr;

use crate::bash;
use crate::macros::*;

pub struct Words {
    words: *mut bash::WordList,
    drop: bool,
}

impl Drop for Words {
    fn drop(&mut self) {
        if self.drop {
            unsafe { bash::dispose_words(self.words) };
        }
    }
}

impl<'a> IntoIterator for &'a Words {
    type Item = &'a str;
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
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.words.map(|w| unsafe {
            self.words = w.next.as_ref();
            let word = (*w.word).word;
            CStr::from_ptr(word).to_str().unwrap()
        })
    }
}

/// Support conversion from a given object into a [`Words`].
pub trait IntoWords {
    /// Convert a given object into a [`Words`].
    fn into_words(self, drop: bool) -> Words;
}

impl IntoWords for *mut bash::WordList {
    fn into_words(self, drop: bool) -> Words {
        Words { words: self, drop }
    }
}

impl<'a> FromIterator<&'a str> for Words {
    fn from_iter<I: IntoIterator<Item = &'a str>>(iter: I) -> Self {
        let mut strs = iter_to_array!(iter.into_iter(), str_to_raw);
        let words = unsafe { bash::strvec_to_word_list(strs.as_mut_ptr(), 1, 0) };
        Words { words, drop: true }
    }
}

impl From<&Words> for *mut bash::WordList {
    fn from(val: &Words) -> Self {
        val.words
    }
}
