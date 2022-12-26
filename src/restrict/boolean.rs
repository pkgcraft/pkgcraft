/// Create a restriction enum injected with boolean variants.
macro_rules! restrict_with_boolean {
   ($name:ident, $($variants:tt)*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variants)*
            And(Vec<Box<Self>>),
            Or(Vec<Box<Self>>),
            Xor(Vec<Box<Self>>),
            Not(Box<Self>),
        }
   }
}
pub(crate) use restrict_with_boolean;

/// Implement restriction matching for a type with injected boolean variants.
macro_rules! restrict_match_boolean {
   ($r:expr, $obj:expr, $($matcher:pat $(if $pred:expr)* => $result:expr,)+) => {
       match $r {
           $($matcher $(if $pred)* => $result,)+
            Self::And(vals) => vals.iter().all(|r| r.matches($obj)),
            Self::Or(vals) => vals.iter().any(|r| r.matches($obj)),
            Self::Xor(vals) => {
                let mut curr: Option<bool>;
                let mut prev: Option<bool> = None;
                for r in vals.iter() {
                    curr = Some(r.matches($obj));
                    if prev.is_some() && curr != prev {
                        return true;
                    }
                    prev = curr
                }
                false
            },
            Self::Not(r) => !r.matches($obj),
       }
   }
}
pub(crate) use restrict_match_boolean;

/// Implement boolean restriction conversions for injected boolean variants.
macro_rules! restrict_impl_boolean {
    ($type:ty) => {
        pub fn and<I, T>(iter: I) -> Self
        where
            I: IntoIterator<Item = T>,
            T: Into<$type>,
        {
            let mut restricts = vec![];
            for r in iter.into_iter().map(Into::into) {
                match r {
                    Self::And(vals) => restricts.extend(vals),
                    _ => restricts.push(Box::new(r)),
                }
            }
            Self::And(restricts)
        }

        pub fn or<I, T>(iter: I) -> Self
        where
            I: IntoIterator<Item = T>,
            T: Into<$type>,
        {
            let mut restricts = vec![];
            for r in iter.into_iter().map(Into::into) {
                match r {
                    Self::Or(vals) => restricts.extend(vals),
                    _ => restricts.push(Box::new(r)),
                }
            }
            Self::Or(restricts)
        }

        pub fn xor<I, T>(iter: I) -> Self
        where
            I: IntoIterator<Item = T>,
            T: Into<$type>,
        {
            let mut restricts = vec![];
            for r in iter.into_iter().map(Into::into) {
                match r {
                    Self::Xor(vals) => restricts.extend(vals),
                    _ => restricts.push(Box::new(r)),
                }
            }
            Self::Xor(restricts)
        }

        pub fn not<T>(obj: T) -> Self
        where
            T: Into<$type>,
        {
            Self::Not(Box::new(obj.into()))
        }
    };
}
pub(crate) use restrict_impl_boolean;

macro_rules! restrict_ops_boolean {
    ($type:ty) => {
        impl std::ops::BitAnd for $type {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self::Output {
                Self::and([self, rhs])
            }
        }

        impl std::ops::BitOr for $type {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self::Output {
                Self::or([self, rhs])
            }
        }

        impl std::ops::BitXor for $type {
            type Output = Self;

            fn bitxor(self, rhs: Self) -> Self::Output {
                Self::xor([self, rhs])
            }
        }

        impl std::ops::Not for $type {
            type Output = Self;

            fn not(self) -> Self::Output {
                Self::Not(Box::new(self))
            }
        }
    };
}
pub(crate) use restrict_ops_boolean;

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom::Atom;
    use crate::restrict::atom::Restrict as AtomRestrict;
    use crate::restrict::{Restrict as BaseRestrict, Restriction};

    #[test]
    fn test_and_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat = AtomRestrict::category("cat");
        let pkg = AtomRestrict::package("pkg");
        let r = BaseRestrict::and([cat, pkg]);
        assert!(r.matches(&a));

        // one matched and one unmatched restriction
        let cat = AtomRestrict::category("cat");
        let pkg = AtomRestrict::package("pkga");
        let r = BaseRestrict::and([cat, pkg]);
        assert!(!(r.matches(&a)));

        // matching against two atoms
        let a1 = Atom::from_str("cat/pkg1").unwrap();
        let a2 = Atom::from_str("cat/pkg2").unwrap();
        let r = BaseRestrict::and([&a1, &a2]);
        assert!(!(r.matches(&a1)));
        assert!(!(r.matches(&a2)));
    }

    #[test]
    fn test_or_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat = AtomRestrict::category("cat");
        let pkg = AtomRestrict::package("pkg");
        let r = BaseRestrict::or([cat, pkg]);
        assert!(r.matches(&a));

        // one matched and one unmatched restriction
        let cat = AtomRestrict::category("cat");
        let pkg = AtomRestrict::package("pkga");
        let r = BaseRestrict::or([cat, pkg]);
        assert!(r.matches(&a));

        // matching against two atoms
        let a1 = Atom::from_str("cat/pkg1").unwrap();
        let a2 = Atom::from_str("cat/pkg2").unwrap();
        let r = BaseRestrict::or([&a1, &a2]);
        assert!(r.matches(&a1));
        assert!(r.matches(&a2));
    }

    #[test]
    fn test_xor_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();

        let cat = AtomRestrict::category("cat");
        let pkg = AtomRestrict::package("pkg");
        let nover = AtomRestrict::Version(None);

        // two matches
        let r = BaseRestrict::xor([cat.clone(), pkg.clone()]);
        assert!(!(r.matches(&a)));

        // three matches
        let r = BaseRestrict::xor([cat, pkg, nover.clone()]);
        assert!(!(r.matches(&a)));

        let cat = AtomRestrict::category("cat");
        let pkg = AtomRestrict::package("pkga");
        let ver = AtomRestrict::version("1").unwrap();

        // one matched and one unmatched
        let r = BaseRestrict::xor([cat.clone(), pkg.clone()]);
        assert!(r.matches(&a));

        // one matched and two unmatched
        let r = BaseRestrict::xor([cat.clone(), pkg.clone(), ver]);
        assert!(r.matches(&a));

        // two matched and one unmatched
        let r = BaseRestrict::xor([cat, pkg, nover]);
        assert!(r.matches(&a));

        let a1 = Atom::from_str("cat/pkg1").unwrap();
        let a2 = Atom::from_str("cat/pkg2").unwrap();
        let a3 = Atom::from_str("cat/pkg3").unwrap();

        // two non-matches
        let r = BaseRestrict::xor([&a1, &a2]);
        assert!(!(r.matches(&a)));

        // three non-matches
        let r = BaseRestrict::xor([&a1, &a2, &a3]);
        assert!(!(r.matches(&a)));
    }

    #[test]
    fn test_not_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let r: BaseRestrict = AtomRestrict::category("cat1").into();

        // restrict doesn't match
        assert!(!(r.matches(&a)));

        // inverse matches
        assert!(!r.matches(&a));
    }
}
