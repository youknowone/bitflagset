use alloc::boxed::Box;
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::ops::Deref;
use core::sync::atomic::Ordering;
use num_traits::{PrimInt, Zero};
use radium::Radium;

use super::atomic_slice::AtomicBitSlice;
use super::boxed::BoxedBitSet;

/// Heap-allocated atomic bitset with dynamically sized storage.
///
/// `AtomicBoxedBitSet<A, V>` is the atomic counterpart of
/// [`BoxedBitSet`](super::BoxedBitSet). It owns a `Box<[A]>` of atomic words
/// and delegates common operations to [`AtomicBitSlice<A, V>`] via `Deref`.
///
/// # Atomicity guarantees
///
/// Each individual method (`insert`, `remove`, `contains`, ...) performs atomic
/// operations **per word**. The bitset as a whole is **not** a single atomic
/// unit — concurrent modifications to bits in different words are independent
/// atomic operations with no cross-word transactional guarantee. See
/// [`AtomicBitSlice`] for details.
pub struct AtomicBoxedBitSet<A, V>(Box<[A]>, PhantomData<V>);

impl<A: Radium, V> core::fmt::Debug for AtomicBoxedBitSet<A, V>
where
    A::Item: PrimInt + core::ops::BitAndAssign,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("AtomicBoxedBitSet")?;
        core::fmt::Debug::fmt(&**self, f)
    }
}

impl<A: Radium, V> core::fmt::Display for AtomicBoxedBitSet<A, V>
where
    A::Item: PrimInt + core::ops::BitAndAssign,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&**self, f)
    }
}

impl<A: Radium, V> Deref for AtomicBoxedBitSet<A, V>
where
    A::Item: PrimInt,
{
    type Target = AtomicBitSlice<A, V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        AtomicBitSlice::from_slice_ref(&self.0)
    }
}

impl<A: Radium, V> Hash for AtomicBoxedBitSet<A, V>
where
    A::Item: Hash + PrimInt,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

impl<A: Radium, V> AtomicBoxedBitSet<A, V>
where
    A::Item: PrimInt,
{
    pub fn from_boxed_slice(store: Box<[A]>) -> Self {
        Self(store, PhantomData)
    }
}

impl<A: Radium + Default, V> AtomicBoxedBitSet<A, V>
where
    A::Item: PrimInt,
{
    pub fn with_capacity(bits: usize) -> Self {
        let bits_per = core::mem::size_of::<A>() * 8;
        let store_size = bits.div_ceil(bits_per);
        let mut store = Vec::new();
        store.resize_with(store_size, A::default);
        Self(store.into_boxed_slice(), PhantomData)
    }
}

impl<A: Radium, V> AtomicBoxedBitSet<A, V>
where
    A::Item: PrimInt,
{
    /// Atomically drain all bits, returning a non-atomic [`BoxedBitSet`]
    /// containing the drained values. The source is left empty.
    ///
    /// Each word is swapped independently — this is NOT an atomic snapshot
    /// of the entire bitset.
    pub fn drain(&self) -> BoxedBitSet<A::Item, V>
    where
        A::Item: Default,
    {
        let mut drained = BoxedBitSet::<A::Item, V>::with_capacity(self.capacity());
        for (source, target) in self.0.iter().zip(drained.as_raw_mut_slice().iter_mut()) {
            *target = source.swap(A::Item::zero(), Ordering::AcqRel);
        }
        drained
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use core::sync::atomic::AtomicU64;
    use proptest::prelude::*;

    #[test]
    fn test_boxed_basic() {
        let bs = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(256);
        assert!(bs.is_empty());
        assert_eq!(bs.len(), 0);
        assert_eq!(bs.capacity(), 256);

        assert!(bs.insert(0));
        assert!(!bs.insert(0));
        assert!(bs.insert(63));
        assert!(bs.insert(64));
        assert!(bs.insert(200));

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

        bs.clear();
        assert!(bs.is_empty());
        assert_eq!(bs.len(), 0);
    }

    #[test]
    fn test_boxed_boundary() {
        // 256 bits
        let bs = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(256);
        bs.insert(0);
        bs.insert(255);
        assert_eq!(bs.len(), 2);
        assert!(bs.contains(&0));
        assert!(bs.contains(&255));

        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 255]);

        bs.remove(255);
        assert!(!bs.contains(&255));

        // 65536 bits
        let bs = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(65536);
        bs.insert(0);
        bs.insert(65535);
        bs.insert(32768);
        assert_eq!(bs.len(), 3);

        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 32768, 65535]);

        bs.remove(65535);
        assert!(!bs.contains(&65535));
        assert_eq!(bs.len(), 2);
    }

    #[test]
    fn test_boxed_drain() {
        let bs = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(256);
        bs.insert(3);
        bs.insert(100);
        bs.insert(200);

        let drained = bs.drain();
        assert!(bs.is_empty());

        let items: Vec<usize> = drained.iter().collect();
        assert_eq!(items, vec![3, 100, 200]);
    }

    #[test]
    fn test_boxed_set_relations() {
        let a = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(256);
        a.insert(1);
        a.insert(65);

        let b = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(256);
        b.insert(1);
        b.insert(65);
        b.insert(200);

        let c = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(256);
        c.insert(2);
        c.insert(66);

        assert!(a.is_subset(&b));
        assert!(!b.is_subset(&a));
        assert!(b.is_superset(&a));
        assert!(!a.is_superset(&b));
        assert!(a.is_disjoint(&c));
        assert!(!a.is_disjoint(&b));
    }

    #[test]
    fn test_mismatched_capacity_subset() {
        // small ⊂ large: small has bits only in shared range
        let small = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(64);
        small.insert(1);
        let large = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(256);
        large.insert(1);
        large.insert(200);
        assert!(small.is_subset(&large));
        assert!(!large.is_subset(&small)); // large has bit 200 beyond small's range

        // small with bits set is NOT subset of empty large
        let s2 = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(128);
        s2.insert(100);
        let l2 = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(64);
        assert!(!s2.is_subset(&l2)); // bit 100 is beyond l2's range

        // empty is always subset
        let empty = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(64);
        assert!(empty.is_subset(&large));
        assert!(empty.is_subset(&small));
    }

    #[test]
    fn test_mismatched_capacity_disjoint() {
        let a = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(64);
        a.insert(1);
        let b = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(256);
        b.insert(200); // beyond a's range, no overlap
        assert!(a.is_disjoint(&b));

        b.insert(1); // now overlaps
        assert!(!a.is_disjoint(&b));
    }

    #[test]
    fn test_boxed_toggle() {
        let bs = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(256);
        bs.insert(10);
        bs.toggle(10);
        assert!(!bs.contains(&10));
        bs.toggle(10);
        assert!(bs.contains(&10));
        bs.toggle(200);
        assert!(bs.contains(&200));
    }

    proptest! {
        #[test]
        fn parity_atomic_boxed_65536(ops in prop::collection::vec((any::<bool>(), 0..65536usize), 0..200)) {
            let atomic = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(65536);
            let mut plain = BoxedBitSet::<u64, usize>::with_capacity(65536);

            for &(insert, idx) in &ops {
                if insert {
                    atomic.insert(idx);
                    plain.insert(idx);
                } else {
                    atomic.remove(idx);
                    plain.remove(idx);
                }
            }

            prop_assert_eq!(atomic.len(), plain.len(), "len mismatch");
            prop_assert_eq!(atomic.is_empty(), plain.is_empty(), "is_empty mismatch");

            let atomic_bits: Vec<usize> = atomic.iter().collect();
            let plain_bits: Vec<usize> = plain.iter().collect();
            prop_assert_eq!(atomic_bits, plain_bits, "iter mismatch");
        }
    }
}
