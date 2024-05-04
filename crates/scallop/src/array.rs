use std::ffi::CStr;
use std::marker::PhantomData;

use crate::variables::{find_variable, Attr};
use crate::{bash, Error};

/// Wrapper type for bash arrays.
pub struct Array<'a> {
    inner: *mut bash::Array,
    len: usize,
    phantom: PhantomData<&'a mut bash::Array>,
}

impl<'a> Array<'a> {
    /// Create a new Array from an existing bash variable.
    pub fn from<S: AsRef<str>>(name: S) -> crate::Result<Self> {
        let name = name.as_ref();
        let ptr = match find_variable!(name) {
            None => Err(Error::Base(format!("undefined variable: {name}"))),
            Some(v) => {
                if (v.attributes as u32 & Attr::ARRAY.bits()) != 0 {
                    Ok(v.value as *mut bash::Array)
                } else {
                    Err(Error::Base(format!("variable is not an array: {name}")))
                }
            }
        }?;

        Ok(Self {
            inner: ptr,
            len: unsafe { (*ptr).num_elements.try_into().unwrap() },
            phantom: PhantomData,
        })
    }

    /// Return a shared iterator for the array.
    pub fn iter(&self) -> ArrayIter {
        self.into_iter()
    }

    /// Return the length of the array.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return true if the array is empty, otherwise false.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<'a> IntoIterator for &'a Array<'a> {
    type Item = &'a str;
    type IntoIter = ArrayIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        unsafe {
            let head = (*self.inner).head;
            ArrayIter {
                head,
                element: head,
                phantom: PhantomData,
            }
        }
    }
}

/// Borrowed iterator for bash arrays.
pub struct ArrayIter<'a> {
    head: *mut bash::array_element,
    element: *mut bash::array_element,
    phantom: PhantomData<&'a mut bash::Array>,
}

impl<'a> Iterator for ArrayIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            self.element = (*self.element).next;
            if self.element != self.head {
                Some(CStr::from_ptr((*self.element).value).to_str().unwrap())
            } else {
                None
            }
        }
    }
}

impl<'a> IntoIterator for Array<'a> {
    type Item = String;
    type IntoIter = ArrayIntoIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        unsafe {
            let head = (*self.inner).head;
            let iter = ArrayIter {
                head,
                element: head,
                phantom: PhantomData,
            };
            ArrayIntoIter { iter }
        }
    }
}

/// Owned iterator for bash arrays.
pub struct ArrayIntoIter<'a> {
    iter: ArrayIter<'a>,
}

impl<'a> Iterator for ArrayIntoIter<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|s| s.to_string())
    }
}

/// Provide access to bash's $PIPESTATUS shell variable.
pub struct PipeStatus {
    statuses: Vec<i32>,
}

impl PipeStatus {
    /// Get the current value for $PIPESTATUS.
    pub fn get() -> Self {
        let statuses = match Array::from("PIPESTATUS") {
            Ok(array) => array.iter().map(|s| s.parse().unwrap_or(-1)).collect(),
            Err(_) => Default::default(),
        };
        Self { statuses }
    }

    /// Determine if a process failed in the related pipeline.
    pub fn failed(&self) -> bool {
        self.statuses.iter().any(|s| *s != 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::source;

    use super::*;

    #[test]
    fn len_and_is_empty() {
        // empty array
        source::string("ARRAY=()").unwrap();
        let array = Array::from("ARRAY").unwrap();
        assert_eq!(array.len(), 0);
        assert!(array.is_empty());

        // non-empty array
        source::string("ARRAY=( 1 2 3 )").unwrap();
        let array = Array::from("ARRAY").unwrap();
        assert_eq!(array.len(), 3);
        assert!(!array.is_empty());
    }
}
