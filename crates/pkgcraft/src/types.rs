use std::fmt::Debug;
use std::hash::Hash;

mod deque;
pub use deque::Deque;
mod ordered_map;
pub use ordered_map::OrderedMap;
mod ordered_set;
pub use ordered_set::OrderedSet;
mod sorted_set;
pub use sorted_set::SortedSet;

pub trait Ordered: Debug + Ord + Clone + Hash {}
impl<T> Ordered for T where T: Debug + Ord + Clone + Hash {}

macro_rules! make_set_traits {
    ($($x:ty),+) => {$(
        impl<T: $crate::types::Ordered> std::ops::BitAnd<&$x> for &$x {
            type Output = $x;

            fn bitand(self, other: &$x) -> Self::Output {
                (&self.0 & &other.0).into()
            }
        }

        impl<T: $crate::types::Ordered> std::ops::BitAndAssign<&$x> for $x {
            fn bitand_assign(&mut self, other: &$x) {
                self.0 = &self.0 & &other.0;
            }
        }

        impl<T: $crate::types::Ordered> std::ops::BitAndAssign<$x> for $x {
            fn bitand_assign(&mut self, other: $x) {
                self.0 = &self.0 & &other.0;
            }
        }

        impl<T: $crate::types::Ordered> std::ops::BitOr<&$x> for &$x {
            type Output = $x;

            fn bitor(self, other: &$x) -> Self::Output {
                (&self.0 | &other.0).into()
            }
        }

        impl<T: $crate::types::Ordered> std::ops::BitOrAssign<&$x> for $x {
            fn bitor_assign(&mut self, other: &$x) {
                self.0 = &self.0 | &other.0;
            }
        }

        impl<T: $crate::types::Ordered> std::ops::BitOrAssign<$x> for $x {
            fn bitor_assign(&mut self, other: $x) {
                self.0 = &self.0 | &other.0;
            }
        }

        impl<T: $crate::types::Ordered> std::ops::BitXor<&$x> for &$x {
            type Output = $x;

            fn bitxor(self, other: &$x) -> Self::Output {
                (&self.0 ^ &other.0).into()
            }
        }

        impl<T: $crate::types::Ordered> std::ops::BitXorAssign<&$x> for $x {
            fn bitxor_assign(&mut self, other: &$x) {
                self.0 = &self.0 ^ &other.0;
            }
        }

        impl<T: $crate::types::Ordered> std::ops::BitXorAssign<$x> for $x {
            fn bitxor_assign(&mut self, other: $x) {
                self.0 = &self.0 ^ &other.0;
            }
        }

        impl<T: $crate::types::Ordered> std::ops::Sub<&$x> for &$x {
            type Output = $x;

            fn sub(self, other: &$x) -> Self::Output {
                (&self.0 - &other.0).into()
            }
        }

        impl<T: $crate::types::Ordered> std::ops::SubAssign<&$x> for $x {
            fn sub_assign(&mut self, other: &$x) {
                self.0 = &self.0 - &other.0;
            }
        }

        impl<T: $crate::types::Ordered> std::ops::SubAssign<$x> for $x {
            fn sub_assign(&mut self, other: $x) {
                self.0 = &self.0 - &other.0;
            }
        }
    )+};
}
use make_set_traits;
