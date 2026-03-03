use alloc::boxed::Box;
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use num_traits::PrimInt;

use super::slice::BitSlice;

pub struct BoxedBitSet<A, V>(Box<[A]>, PhantomData<V>);

impl<A: Clone, V> Clone for BoxedBitSet<A, V> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<A, V> Deref for BoxedBitSet<A, V> {
    type Target = BitSlice<A, V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        BitSlice::from_slice_ref(&self.0)
    }
}

impl<A, V> DerefMut for BoxedBitSet<A, V> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        BitSlice::from_slice_mut(&mut self.0)
    }
}

impl<A, V> BoxedBitSet<A, V> {
    pub fn from_boxed_slice(store: Box<[A]>) -> Self {
        Self(store, PhantomData)
    }

    pub fn with_capacity(bits: usize) -> Self
    where
        A: Default,
    {
        let store_size = bits.div_ceil(core::mem::size_of::<A>() * 8);
        let mut store = Vec::new();
        store.resize_with(store_size, A::default);
        Self(store.into_boxed_slice(), PhantomData)
    }

    #[inline]
    pub fn as_raw_slice(&self) -> &[A] {
        &self.0
    }

    #[inline]
    pub fn as_raw_mut_slice(&mut self) -> &mut [A] {
        &mut self.0
    }
}

impl<A: PrimInt, V> PartialEq for BoxedBitSet<A, V> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<A: PrimInt, V> Eq for BoxedBitSet<A, V> {}

impl<A: PrimInt + Hash, V> Hash for BoxedBitSet<A, V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

impl<A: PrimInt + core::ops::BitAndAssign, V> core::fmt::Debug for BoxedBitSet<A, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("BoxedBitSet")?;
        core::fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, A: PrimInt + core::ops::BitAndAssign, V: TryFrom<usize>> IntoIterator
    for &'a BoxedBitSet<A, V>
{
    type Item = V;
    type IntoIter = super::slice::BitSliceIter<'a, A, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<A: PrimInt + core::ops::BitAndAssign, V: TryFrom<usize>> IntoIterator for BoxedBitSet<A, V> {
    type Item = V;
    type IntoIter = BoxedBitSetIntoIter<A, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        super::slice::WordSetIter::new(self.0)
    }
}

/// Owned iterator over set bit positions in a `BoxedBitSet<A, V>`.
pub type BoxedBitSetIntoIter<T, V> = super::slice::WordSetIter<Box<[T]>, T, V>;

impl<A: PrimInt, V> core::ops::BitOr for BoxedBitSet<A, V> {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        let (mut longer, shorter) = if self.0.len() >= rhs.0.len() {
            (self, rhs)
        } else {
            (rhs, self)
        };
        for (a, b) in longer.0.iter_mut().zip(shorter.0.iter()) {
            *a = *a | *b;
        }
        longer
    }
}

impl<A: PrimInt, V> core::ops::BitOrAssign for BoxedBitSet<A, V> {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        for (a, b) in self.0.iter_mut().zip(rhs.0.iter()) {
            *a = *a | *b;
        }
    }
}

impl<A: PrimInt, V> core::ops::BitAnd for BoxedBitSet<A, V> {
    type Output = Self;
    #[inline]
    fn bitand(mut self, rhs: Self) -> Self {
        for (a, b) in self.0.iter_mut().zip(rhs.0.iter()) {
            *a = *a & *b;
        }
        // Words in self beyond rhs become zero (AND with implicit zero)
        let tail_start = rhs.0.len().min(self.0.len());
        for w in self.0[tail_start..].iter_mut() {
            *w = A::zero();
        }
        self
    }
}

impl<A: PrimInt, V> core::ops::BitAndAssign for BoxedBitSet<A, V> {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        let tail_start = rhs.0.len().min(self.0.len());
        for (a, b) in self.0.iter_mut().zip(rhs.0.iter()) {
            *a = *a & *b;
        }
        for w in self.0[tail_start..].iter_mut() {
            *w = A::zero();
        }
    }
}

impl<A: PrimInt, V> core::ops::BitXor for BoxedBitSet<A, V> {
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        let (mut longer, shorter) = if self.0.len() >= rhs.0.len() {
            (self, rhs)
        } else {
            (rhs, self)
        };
        for (a, b) in longer.0.iter_mut().zip(shorter.0.iter()) {
            *a = *a ^ *b;
        }
        longer
    }
}

impl<A: PrimInt, V> core::ops::BitXorAssign for BoxedBitSet<A, V> {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        for (a, b) in self.0.iter_mut().zip(rhs.0.iter()) {
            *a = *a ^ *b;
        }
    }
}

impl<A: PrimInt, V> core::ops::Not for BoxedBitSet<A, V> {
    type Output = Self;
    #[inline]
    fn not(mut self) -> Self {
        for w in self.0.iter_mut() {
            *w = !*w;
        }
        self
    }
}

impl<A: PrimInt, V> core::ops::Sub for BoxedBitSet<A, V> {
    type Output = Self;
    #[inline]
    fn sub(mut self, rhs: Self) -> Self {
        for (a, b) in self.0.iter_mut().zip(rhs.0.iter()) {
            *a = *a & !*b;
        }
        self
    }
}

impl<A: PrimInt, V> core::ops::SubAssign for BoxedBitSet<A, V> {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        for (a, b) in self.0.iter_mut().zip(rhs.0.iter()) {
            *a = *a & !*b;
        }
    }
}

impl<A: PrimInt, V> FromIterator<BoxedBitSet<A, V>> for BoxedBitSet<A, V> {
    fn from_iter<I: IntoIterator<Item = Self>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        let Some(first) = iter.next() else {
            return BoxedBitSet::from_boxed_slice(Box::new([]) as Box<[A]>);
        };
        iter.fold(first, |acc, bs| acc | bs)
    }
}

impl<A: PrimInt, V> core::iter::Extend<BoxedBitSet<A, V>> for BoxedBitSet<A, V> {
    fn extend<I: IntoIterator<Item = Self>>(&mut self, iter: I) {
        for bs in iter {
            for (a, b) in self.0.iter_mut().zip(bs.0.iter()) {
                *a = *a | *b;
            }
        }
    }
}

#[cfg(feature = "bitvec")]
impl<A: bitvec::store::BitStore + Copy, V> BoxedBitSet<A, V> {
    pub fn from_bitvec_box(raw: bitvec::boxed::BitBox<A>) -> Self {
        Self(raw.as_raw_slice().to_vec().into_boxed_slice(), PhantomData)
    }

    pub fn into_bitvec_box(self) -> bitvec::boxed::BitBox<A> {
        bitvec::boxed::BitBox::from_boxed_slice(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use bitvec::order::Lsb0;
    use proptest::prelude::*;

    #[test]
    fn test_basic() {
        let mut bs = BoxedBitSet::<u64, usize>::with_capacity(256);
        assert!(bs.is_empty());
        assert_eq!(bs.capacity(), 256);

        bs.insert(0);
        bs.insert(63);
        bs.insert(64);
        bs.insert(200);
        assert!(!bs.is_empty());
        assert_eq!(bs.len(), 4);

        assert!(bs.contains(&0));
        assert!(bs.contains(&63));
        assert!(bs.contains(&64));
        assert!(bs.contains(&200));
        assert!(!bs.contains(&1));

        assert!(bs.remove(63));
        assert!(!bs.remove(63));
        assert_eq!(bs.len(), 3);

        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 64, 200]);

        bs.clear();
        assert!(bs.is_empty());
    }

    /// Apply the same sequence of insert/remove to both BoxedBitSet and bitvec BitVec,
    /// then assert all observable state matches.
    fn apply_and_compare(capacity: usize, ops: &[(bool, usize)]) {
        let mut ours = BoxedBitSet::<u64, usize>::with_capacity(capacity);
        let mut bv = bitvec::vec::BitVec::<u64, Lsb0>::repeat(false, capacity);

        for &(insert, idx) in ops {
            if insert {
                ours.insert(idx);
                bv.set(idx, true);
            } else {
                ours.remove(idx);
                bv.set(idx, false);
            }
        }

        // len
        assert_eq!(ours.len(), bv.count_ones(), "len mismatch");

        // is_empty
        assert_eq!(ours.is_empty(), bv.not_any(), "is_empty mismatch");

        // contains — check every bit
        for i in 0..capacity {
            assert_eq!(ours.contains(&i), bv[i], "contains mismatch at bit {i}");
        }

        // iter — same set bits in same order
        let ours_bits: Vec<usize> = ours.iter().collect();
        let bv_bits: Vec<usize> = bv.iter_ones().collect();
        assert_eq!(ours_bits, bv_bits, "iter mismatch");
    }

    #[test]
    fn test_boundary_values() {
        // 256 bits, max index = 255
        let mut bs = BoxedBitSet::<u64, usize>::with_capacity(256);
        bs.insert(0);
        bs.insert(255);
        assert_eq!(bs.len(), 2);
        assert!(bs.contains(&0));
        assert!(bs.contains(&255));
        assert!(!bs.contains(&1));
        assert!(!bs.contains(&254));

        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 255]);

        assert!(bs.remove(255));
        assert!(!bs.contains(&255));
        assert_eq!(bs.len(), 1);

        // 65536 bits, max index = 65535
        let mut bs = BoxedBitSet::<u64, usize>::with_capacity(65536);
        bs.insert(0);
        bs.insert(65535);
        bs.insert(32768);
        assert_eq!(bs.len(), 3);
        assert!(bs.contains(&0));
        assert!(bs.contains(&65535));
        assert!(bs.contains(&32768));

        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 32768, 65535]);

        assert!(bs.remove(65535));
        assert!(!bs.contains(&65535));
        assert_eq!(bs.len(), 2);
    }

    #[test]
    fn test_set_relations() {
        let mut a = BoxedBitSet::<u64, usize>::with_capacity(256);
        a.insert(1);
        a.insert(65);

        let mut b = BoxedBitSet::<u64, usize>::with_capacity(256);
        b.insert(1);
        b.insert(65);
        b.insert(200);

        let mut c = BoxedBitSet::<u64, usize>::with_capacity(256);
        c.insert(2);
        c.insert(66);

        assert!(a.is_subset(&b));
        assert!(!b.is_subset(&a));
        assert!(b.is_superset(&a));
        assert!(a.is_disjoint(&c));
        assert!(!a.is_disjoint(&b));
    }

    #[test]
    fn test_mismatched_capacity_relations() {
        let mut small = BoxedBitSet::<u64, usize>::with_capacity(64);
        small.insert(1);

        let mut large = BoxedBitSet::<u64, usize>::with_capacity(256);
        large.insert(1);
        large.insert(200);

        assert!(small.is_subset(&large));
        assert!(!large.is_subset(&small));

        let mut s2 = BoxedBitSet::<u64, usize>::with_capacity(128);
        s2.insert(100);
        let l2 = BoxedBitSet::<u64, usize>::with_capacity(64);
        assert!(!s2.is_subset(&l2));

        let empty = BoxedBitSet::<u64, usize>::with_capacity(64);
        assert!(empty.is_subset(&large));
    }

    #[test]
    fn test_bit_operators() {
        let mut a = BoxedBitSet::<u64, usize>::with_capacity(256);
        a.insert(1);
        a.insert(100);

        let mut b = BoxedBitSet::<u64, usize>::with_capacity(256);
        b.insert(100);
        b.insert(200);

        let union = a.clone() | b.clone();
        assert_eq!(union.len(), 3);
        assert!(union.contains(&1));
        assert!(union.contains(&100));
        assert!(union.contains(&200));

        let intersection = a.clone() & b.clone();
        assert_eq!(intersection.len(), 1);
        assert!(intersection.contains(&100));

        let diff = a.clone() - b.clone();
        assert_eq!(diff.len(), 1);
        assert!(diff.contains(&1));

        let xor = a.clone() ^ b.clone();
        assert_eq!(xor.len(), 2);
        assert!(xor.contains(&1));
        assert!(xor.contains(&200));

        let complement = !a.clone();
        assert!(!complement.contains(&1));
        assert!(!complement.contains(&100));
        assert!(complement.contains(&0));
    }

    #[test]
    fn test_mismatched_capacity_operators() {
        let mut small = BoxedBitSet::<u64, usize>::with_capacity(64);
        small.insert(1);
        small.insert(10);

        let mut large = BoxedBitSet::<u64, usize>::with_capacity(256);
        large.insert(10);
        large.insert(200);

        // BitOr: preserves all bits from both operands
        let union_sl = small.clone() | large.clone();
        assert_eq!(union_sl.capacity(), 256);
        assert_eq!(union_sl.len(), 3);
        assert!(union_sl.contains(&1));
        assert!(union_sl.contains(&10));
        assert!(union_sl.contains(&200));

        let union_ls = large.clone() | small.clone();
        assert_eq!(union_ls.capacity(), 256);
        assert_eq!(union_ls.len(), 3);
        assert!(union_ls.contains(&1));
        assert!(union_ls.contains(&10));
        assert!(union_ls.contains(&200));

        // BitXor: preserves tail bits from the longer operand
        let xor_sl = small.clone() ^ large.clone();
        assert_eq!(xor_sl.capacity(), 256);
        assert_eq!(xor_sl.len(), 2);
        assert!(xor_sl.contains(&1));
        assert!(xor_sl.contains(&200));

        let xor_ls = large.clone() ^ small.clone();
        assert_eq!(xor_ls.capacity(), 256);
        assert_eq!(xor_ls.len(), 2);
        assert!(xor_ls.contains(&1));
        assert!(xor_ls.contains(&200));

        // BitAnd: result bounded by self's capacity, tail zeroed
        let and_sl = small.clone() & large.clone();
        assert_eq!(and_sl.capacity(), 64);
        assert_eq!(and_sl.len(), 1);
        assert!(and_sl.contains(&10));

        let and_ls = large.clone() & small.clone();
        assert_eq!(and_ls.capacity(), 256);
        assert_eq!(and_ls.len(), 1);
        assert!(and_ls.contains(&10));

        // Sub: bits beyond rhs remain in self
        let sub_ls = large.clone() - small.clone();
        assert_eq!(sub_ls.len(), 1);
        assert!(sub_ls.contains(&200));

        let sub_sl = small.clone() - large.clone();
        assert_eq!(sub_sl.len(), 1);
        assert!(sub_sl.contains(&1));
    }

    proptest! {
        #[test]
        fn parity_1024(ops in prop::collection::vec((any::<bool>(), 0..1024usize), 0..200)) {
            apply_and_compare(1024, &ops);
        }

        #[test]
        fn parity_65536(ops in prop::collection::vec((any::<bool>(), 0..65536usize), 0..200)) {
            apply_and_compare(65536, &ops);
        }

        #[test]
        fn parity_capacity(bits in 1..4096usize) {
            let ours = BoxedBitSet::<u64, usize>::with_capacity(bits);
            let bv = bitvec::vec::BitVec::<u64, Lsb0>::repeat(false, bits);
            // capacity is rounded up to word boundary for both
            let expected_capacity = bits.div_ceil(64) * 64;
            assert_eq!(ours.capacity(), expected_capacity, "capacity mismatch");
            assert_eq!(bv.len(), bits, "bitvec len");
            // both start empty
            assert!(ours.is_empty());
            assert!(bv.not_any());
        }
    }

    #[test]
    fn test_toggle() {
        let mut bs = BoxedBitSet::<u64, usize>::with_capacity(256);
        bs.insert(10);
        bs.toggle(10);
        assert!(!bs.contains(&10));
        bs.toggle(10);
        assert!(bs.contains(&10));
        bs.toggle(200);
        assert!(bs.contains(&200));
    }

    #[test]
    fn test_from_iter_self() {
        let mut a = BoxedBitSet::<u64, usize>::with_capacity(256);
        a.insert(1);
        let mut b = BoxedBitSet::<u64, usize>::with_capacity(256);
        b.insert(200);

        let merged: BoxedBitSet<u64, usize> = [a, b].into_iter().collect();
        assert!(merged.contains(&1));
        assert!(merged.contains(&200));
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_extend_self() {
        let mut bs = BoxedBitSet::<u64, usize>::with_capacity(256);
        bs.insert(5);

        let mut other = BoxedBitSet::<u64, usize>::with_capacity(256);
        other.insert(100);

        bs.extend([other]);
        assert!(bs.contains(&5));
        assert!(bs.contains(&100));
    }
}
