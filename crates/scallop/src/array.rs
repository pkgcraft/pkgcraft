use std::ffi::{CStr, CString, c_long};
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;

use crate::variables::{Attr, find_variable};
use crate::{Error, bash};

/// Wrapper type for bash arrays.
pub struct Array<'a> {
    inner: *mut bash::Array,
    phantom: PhantomData<&'a mut bash::Array>,
}

impl Array<'_> {
    /// Create a new Array using the given variable name.
    pub fn new<S: AsRef<str>>(name: S) -> Self {
        let cstr = CString::new(name.as_ref()).unwrap();
        unsafe {
            let ptr = bash::make_new_array_variable(cstr.as_ptr() as *mut _);
            Self {
                inner: (*ptr).value as *mut _,
                phantom: PhantomData,
            }
        }
    }

    /// Create a new Array from an existing variable.
    pub fn from<S: AsRef<str>>(name: S) -> crate::Result<Self> {
        let name = name.as_ref();
        let ptr = match find_variable!(name) {
            None => Err(Error::Base(format!("undefined variable: {name}"))),
            Some(v) => {
                if (v.attributes as u32 & Attr::ARRAY.bits()) != 0 {
                    Ok(v.value)
                } else {
                    Err(Error::Base(format!("variable is not an array: {name}")))
                }
            }
        }?;

        Ok(Self {
            inner: ptr as *mut _,
            phantom: PhantomData,
        })
    }

    /// Return a shared iterator for the array.
    pub fn iter(&self) -> ArrayIter<'_> {
        self.into_iter()
    }

    /// Insert an element into the array with a given index value.
    pub fn insert<I, S>(&mut self, index: I, value: S)
    where
        I: Into<c_long>,
        S: AsRef<str>,
    {
        let cstr = CString::new(value.as_ref()).unwrap();
        let cstr = cstr.as_ptr() as *mut _;
        unsafe {
            bash::array_insert(self.inner, index.into(), cstr);
        }
    }

    /// Return a reference to a value at a given index.
    pub fn get<I: Into<c_long>>(&mut self, index: I) -> Option<&str> {
        unsafe {
            let value = bash::array_reference(self.inner, index.into());
            value.as_ref().map(|s| CStr::from_ptr(s).to_str().unwrap())
        }
    }

    /// Remove and return the value at a given index.
    pub fn remove<I: Into<c_long>>(&mut self, index: I) -> Option<String> {
        unsafe {
            let element = bash::array_remove(self.inner, index.into());
            let value = element
                .as_ref()
                .map(|e| CStr::from_ptr(e.value).to_str().unwrap().to_string());
            bash::array_dispose_element(element);
            value
        }
    }

    /// Remove and return the last value if it exists.
    pub fn pop(&mut self) -> Option<String> {
        let index = unsafe { (*self.inner).max_index };
        self.remove(index)
    }

    /// Append a value to the array.
    pub fn push<S: AsRef<str>>(&mut self, value: S) {
        let index = unsafe { (*self.inner).max_index + 1 };
        self.insert(index, value);
    }

    /// Return the length of the array.
    pub fn len(&self) -> usize {
        unsafe { (*self.inner).num_elements.try_into().unwrap() }
    }

    /// Return true if the array is empty, otherwise false.
    pub fn is_empty(&self) -> bool {
        unsafe { (*self.inner).num_elements == 0 }
    }
}

impl<S: AsRef<str>> Extend<S> for Array<'_> {
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = S>,
    {
        for value in iter {
            self.push(value);
        }
    }
}

// Note that this only compares array values and their order, not indices.
impl PartialEq for Array<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl Eq for Array<'_> {}

impl fmt::Debug for Array<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let values: Vec<_> = self.iter().collect();
        write!(f, "Array {{ {values:?} }}")
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

impl Iterator for ArrayIntoIter<'_> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|s| s.to_string())
    }
}

/// Provide access to bash's $PIPESTATUS shell variable.
pub struct PipeStatus(Vec<i32>);

impl PipeStatus {
    /// Get the current value for $PIPESTATUS.
    pub fn get() -> Self {
        let statuses = match Array::from("PIPESTATUS") {
            Ok(array) => array.iter().map(|s| s.parse().unwrap_or(-1)).collect(),
            Err(_) => Default::default(),
        };
        Self(statuses)
    }

    /// Determine if a process failed in the related pipeline.
    pub fn failed(&self) -> bool {
        self.iter().any(|s| *s != 0)
    }
}

impl Deref for PipeStatus {
    type Target = [i32];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::source;

    use super::*;

    #[test]
    fn new() {
        let mut array = Array::new("ARRAY");
        assert_eq!(format!("{array:?}"), "Array { [] }");
        array.push("a");
        assert_eq!(format!("{array:?}"), r#"Array { ["a"] }"#);

        // verify native bash existence
        source::string("[[ ${ARRAY[0]} == a ]]").unwrap();
    }

    #[test]
    fn from() {
        // nonexistent
        assert!(Array::from("ARRAY").is_err());

        // non-array
        source::string("ARRAY=a").unwrap();
        assert!(Array::from("ARRAY").is_err());

        // empty
        source::string("ARRAY=()").unwrap();
        let array = Array::from("ARRAY").unwrap();
        assert_eq!(format!("{array:?}"), "Array { [] }");
        assert_eq!(array.len(), 0);
        assert!(array.is_empty());

        // non-empty
        source::string("ARRAY=( 1 2 3 )").unwrap();
        let array = Array::from("ARRAY").unwrap();
        assert_eq!(format!("{array:?}"), r#"Array { ["1", "2", "3"] }"#);
        assert_eq!(array.len(), 3);
        assert!(!array.is_empty());
    }

    #[test]
    fn eq() {
        // empty
        let mut array1 = Array::new("ARRAY1");
        source::string("ARRAY2=()").unwrap();
        let mut array2 = Array::from("ARRAY2").unwrap();
        assert_eq!(&array1, &array2);

        // non-empty
        array1.push("1");
        array2.push("1");
        assert_eq!(&array1, &array2);

        // non-matching indices
        array1.insert(100, "2");
        array2.insert(101, "2");
        assert_eq!(&array1, &array2);

        // non-matching values
        array1.push("3");
        assert_ne!(&array1, &array2);
    }

    #[test]
    fn iterator() {
        // empty array
        let array = Array::new("ARRAY");
        assert!(array.iter().next().is_none());
        assert!(array.into_iter().next().is_none());

        // non-empty array
        let mut array = Array::new("ARRAY");
        array.extend(["1", "2", "3"]);
        assert_eq!(array.iter().collect::<Vec<_>>(), ["1", "2", "3"]);
        assert_eq!(array.into_iter().collect::<Vec<_>>(), ["1", "2", "3"]);
    }

    #[test]
    fn manipulation() {
        // empty array
        let mut array = Array::new("ARRAY");

        // remove nonexistent
        assert!(array.remove(0).is_none());
        assert!(array.get(0).is_none());
        assert!(array.pop().is_none());

        // push
        array.push("1");
        assert_eq!(array.iter().collect::<Vec<_>>(), ["1"]);
        assert_eq!(array.get(0).unwrap(), "1");

        // insert overriding existing
        array.insert(0, "2");
        assert_eq!(array.iter().collect::<Vec<_>>(), ["2"]);
        assert_eq!(array.get(0).unwrap(), "2");

        // insert new
        array.insert(1, "3");
        assert_eq!(array.iter().collect::<Vec<_>>(), ["2", "3"]);
        assert_eq!(array.get(1).unwrap(), "3");

        // insert non-sequential
        array.insert(100, "5");
        array.insert(99, "4");
        assert_eq!(array.iter().collect::<Vec<_>>(), ["2", "3", "4", "5"]);
        assert_eq!(array.get(99).unwrap(), "4");

        // push starts at the latest index
        array.push("6");
        assert_eq!(array.iter().collect::<Vec<_>>(), ["2", "3", "4", "5", "6"]);
        assert_eq!(array.get(101).unwrap(), "6");

        // remove existing
        assert_eq!(array.remove(0).unwrap(), "2");
        assert!(array.get(0).is_none());
        assert_eq!(array.iter().collect::<Vec<_>>(), ["3", "4", "5", "6"]);

        // pop values
        assert_eq!(array.pop().unwrap(), "6");
        assert_eq!(array.iter().collect::<Vec<_>>(), ["3", "4", "5"]);
    }

    #[test]
    fn extend() {
        let mut array = Array::new("ARRAY");
        array.extend(["1"]);
        assert_eq!(array.iter().collect::<Vec<_>>(), ["1"]);
        array.extend(vec!["2".to_string(), "3".to_string()]);
        assert_eq!(array.iter().collect::<Vec<_>>(), ["1", "2", "3"]);
    }

    #[test]
    fn pipestatus() {
        // nonexistent
        let pipestatus = PipeStatus::get();
        assert!(!pipestatus.failed());
        assert!(pipestatus.is_empty());

        // single success
        source::string("true").ok();
        let pipestatus = PipeStatus::get();
        assert!(!pipestatus.failed());
        assert_eq!(pipestatus.iter().copied().collect::<Vec<_>>(), [0]);

        // single failure
        source::string("false").ok();
        let pipestatus = PipeStatus::get();
        assert!(pipestatus.failed());
        assert_eq!(pipestatus.iter().copied().collect::<Vec<_>>(), [1]);

        // multiple commands
        source::string("true | false | true").ok();
        let pipestatus = PipeStatus::get();
        assert!(pipestatus.failed());
        assert_eq!(pipestatus.iter().copied().collect::<Vec<_>>(), [0, 1, 0]);
    }
}
