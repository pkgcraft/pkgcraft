/// Create a restriction enum injected with boolean variants.
macro_rules! restrict_with_boolean {
   ($name:ident, $($variants:tt)*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variants)*
            False,
            True,
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
            Self::False => false,
            Self::True => true,
            Self::And(vals) => vals.iter().all(|r| r.matches($obj)),
            Self::Or(vals) => vals.iter().any(|r| r.matches($obj)),
            Self::Xor(vals) => {
                let mut curr: Option<bool>;
                let mut prev: Option<bool> = None;
                for r in vals {
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
        pub fn and<I>(iter: I) -> Self
        where
            I: IntoIterator,
            I::Item: Into<$type>,
        {
            let mut restricts = vec![];
            for r in iter.into_iter().map(Into::into) {
                match r {
                    Self::And(vals) => restricts.extend(vals),
                    _ => restricts.push(Box::new(r)),
                }
            }

            match restricts.len() {
                0 => Self::True,
                1 => *restricts.pop().unwrap(),
                _ => Self::And(restricts),
            }
        }

        pub fn or<I>(iter: I) -> Self
        where
            I: IntoIterator,
            I::Item: Into<$type>,
        {
            let mut restricts = vec![];
            for r in iter.into_iter().map(Into::into) {
                match r {
                    Self::Or(vals) => restricts.extend(vals),
                    _ => restricts.push(Box::new(r)),
                }
            }

            match restricts.len() {
                0 => Self::True,
                1 => *restricts.pop().unwrap(),
                _ => Self::Or(restricts),
            }
        }

        pub fn xor<I>(iter: I) -> Self
        where
            I: IntoIterator,
            I::Item: Into<$type>,
        {
            let mut restricts = vec![];
            for r in iter.into_iter().map(Into::into) {
                match r {
                    Self::Xor(vals) => restricts.extend(vals),
                    _ => restricts.push(Box::new(r)),
                }
            }

            match restricts.len() {
                0 => Self::True,
                1 => *restricts.pop().unwrap(),
                _ => Self::Xor(restricts),
            }
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
    use crate::dep::Dep;
    use crate::restrict::dep::Restrict as DepRestrict;
    use crate::restrict::{Restrict as BaseRestrict, Restriction};

    #[test]
    fn and() {
        let d = Dep::try_new("cat/pkg").unwrap();
        let cat = DepRestrict::category("cat");
        let pkg = DepRestrict::package("pkg");
        let r = BaseRestrict::and([cat, pkg]);
        assert!(r.matches(&d));

        // one matched and one unmatched restriction
        let cat = DepRestrict::category("cat");
        let pkg = DepRestrict::package("pkga");
        let r = BaseRestrict::and([cat, pkg]);
        assert!(!(r.matches(&d)));

        // matching against two deps
        let d1 = Dep::try_new("cat/pkg1").unwrap();
        let d2 = Dep::try_new("cat/pkg2").unwrap();
        let r = BaseRestrict::and([&d1, &d2]);
        assert!(!(r.matches(&d1)));
        assert!(!(r.matches(&d2)));
    }

    #[test]
    fn or() {
        let d = Dep::try_new("cat/pkg").unwrap();
        let cat = DepRestrict::category("cat");
        let pkg = DepRestrict::package("pkg");
        let r = BaseRestrict::or([cat, pkg]);
        assert!(r.matches(&d));

        // one matched and one unmatched restriction
        let cat = DepRestrict::category("cat");
        let pkg = DepRestrict::package("pkga");
        let r = BaseRestrict::or([cat, pkg]);
        assert!(r.matches(&d));

        // matching against two deps
        let d1 = Dep::try_new("cat/pkg1").unwrap();
        let d2 = Dep::try_new("cat/pkg2").unwrap();
        let r = BaseRestrict::or([&d1, &d2]);
        assert!(r.matches(&d1));
        assert!(r.matches(&d2));
    }

    #[test]
    fn xor() {
        let d = Dep::try_new("cat/pkg").unwrap();

        let cat = DepRestrict::category("cat");
        let pkg = DepRestrict::package("pkg");
        let nover = DepRestrict::Version(None);

        // two matches
        let r = BaseRestrict::xor([cat.clone(), pkg.clone()]);
        assert!(!(r.matches(&d)));

        // three matches
        let r = BaseRestrict::xor([cat, pkg, nover.clone()]);
        assert!(!(r.matches(&d)));

        let cat = DepRestrict::category("cat");
        let pkg = DepRestrict::package("pkga");
        let ver = DepRestrict::version("1").unwrap();

        // one matched and one unmatched
        let r = BaseRestrict::xor([cat.clone(), pkg.clone()]);
        assert!(r.matches(&d));

        // one matched and two unmatched
        let r = BaseRestrict::xor([cat.clone(), pkg.clone(), ver]);
        assert!(r.matches(&d));

        // two matched and one unmatched
        let r = BaseRestrict::xor([cat, pkg, nover]);
        assert!(r.matches(&d));

        let d1 = Dep::try_new("cat/pkg1").unwrap();
        let d2 = Dep::try_new("cat/pkg2").unwrap();
        let d3 = Dep::try_new("cat/pkg3").unwrap();

        // two non-matches
        let r = BaseRestrict::xor([&d1, &d2]);
        assert!(!(r.matches(&d)));

        // three non-matches
        let r = BaseRestrict::xor([&d1, &d2, &d3]);
        assert!(!(r.matches(&d)));
    }

    #[test]
    fn not() {
        let d = Dep::try_new("cat/pkg").unwrap();
        let r: BaseRestrict = DepRestrict::category("cat1").into();

        // restrict doesn't match
        assert!(!(r.matches(&d)));

        // inverse matches
        assert!(!r.matches(&d));
    }
}
