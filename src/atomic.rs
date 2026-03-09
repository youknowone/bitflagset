use core::{marker::PhantomData, ops::Deref, sync::atomic::Ordering};
use num_traits::{AsPrimitive, One, PrimInt, Zero};
use radium::Radium;

use super::atomic_slice::AtomicBitSlice;
use super::bitset::{BitSet, PrimBitSetIter, PrimStore};

// Sealed marker: only applies to single atomic primitive stores.
// Prevents coherence conflicts with [A; N].
mod sealed {
    use core::sync::atomic::*;
    use radium::Radium;

    pub trait AtomicPrimStore: Radium {
        const ZERO: Self;
    }
    impl AtomicPrimStore for AtomicU8 {
        const ZERO: Self = AtomicU8::new(0);
    }
    impl AtomicPrimStore for AtomicU16 {
        const ZERO: Self = AtomicU16::new(0);
    }
    impl AtomicPrimStore for AtomicU32 {
        const ZERO: Self = AtomicU32::new(0);
    }
    impl AtomicPrimStore for AtomicU64 {
        const ZERO: Self = AtomicU64::new(0);
    }
    impl AtomicPrimStore for AtomicUsize {
        const ZERO: Self = AtomicUsize::new(0);
    }
}
pub use sealed::AtomicPrimStore;

/// Atomic bitset backed by a single atomic primitive or a fixed-size array of atomics.
///
/// # Atomicity guarantees
///
/// For single-primitive stores (e.g. `AtomicBitSet<AtomicU64, V>`), each method
/// operates on one atomic word — all operations are truly atomic with respect to
/// the entire bitset.
///
/// For multi-word stores (e.g. `AtomicBitSet<[AtomicU64; 4], V>`), each individual
/// operation (`insert`, `remove`, `contains`, ...) is atomic **per word**, but
/// the bitset as a whole is **not** a single atomic unit. Concurrent modifications
/// to bits in the **same word** are correctly synchronized. Modifications to bits
/// in **different words** are independent atomic operations with no cross-word
/// transactional guarantee. Composite queries like `len()`, `iter()`, or
/// `is_subset()` may observe a mix of old and new state across words under
/// concurrent mutation.
pub struct AtomicBitSet<A, V>(pub(crate) A, PhantomData<V>);

impl<A: AtomicPrimStore, V> core::fmt::Debug for AtomicBitSet<A, V>
where
    A::Item: PrimInt,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut formatter = f.debug_tuple("AtomicBitSet");
        let store = self.0.load(Ordering::Relaxed);
        let bits = core::mem::size_of::<A>() * 8;
        for idx in 0..bits {
            if store & A::Item::one().unsigned_shl(idx as u32) != A::Item::zero() {
                formatter.field(&idx);
            }
        }
        formatter.finish()
    }
}

impl<A: AtomicPrimStore, V> Default for AtomicBitSet<A, V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<A: AtomicPrimStore, V> AtomicBitSet<A, V> {
    const BITS: usize = core::mem::size_of::<A>() * 8;
    const ZERO: Self = Self(A::ZERO, PhantomData);

    #[inline]
    pub const fn new() -> Self {
        Self::ZERO
    }

    #[inline]
    pub fn from_element(elem: V) -> Self
    where
        V: AsPrimitive<usize>,
        A::Item: PrimInt + radium::marker::BitOps,
    {
        let ret = Self::new();
        ret.set(elem, true);
        ret
    }

    #[inline]
    pub fn from_bits(bits: A) -> Self {
        Self(bits, PhantomData)
    }

    #[inline]
    pub fn as_bits(&self) -> &A {
        &self.0
    }

    #[inline]
    pub fn into_bits(self) -> A {
        self.0
    }

    #[inline]
    pub fn len(&self) -> usize
    where
        A::Item: PrimInt,
    {
        self.0.load(Ordering::Relaxed).count_ones() as usize
    }

    #[inline]
    pub fn is_empty(&self) -> bool
    where
        A::Item: PrimInt,
    {
        self.0.load(Ordering::Relaxed).is_zero()
    }

    #[inline]
    pub fn first(&self) -> Option<V>
    where
        V: TryFrom<u8>,
        A::Item: PrimInt,
    {
        let store = self.0.load(Ordering::Relaxed);
        if store.is_zero() {
            return None;
        }
        let converted = V::try_from(store.trailing_zeros() as u8);
        debug_assert!(converted.is_ok());
        Some(match converted {
            Ok(value) => value,
            Err(_) => unsafe { core::hint::unreachable_unchecked() },
        })
    }

    #[inline]
    pub fn last(&self) -> Option<V>
    where
        V: TryFrom<u8>,
        A::Item: PrimInt,
    {
        let store = self.0.load(Ordering::Relaxed);
        if store.is_zero() {
            return None;
        }
        let bit = Self::BITS - 1 - store.leading_zeros() as usize;
        let converted = V::try_from(bit as u8);
        debug_assert!(converted.is_ok());
        Some(match converted {
            Ok(value) => value,
            Err(_) => unsafe { core::hint::unreachable_unchecked() },
        })
    }

    #[inline]
    pub fn pop_first(&self) -> Option<V>
    where
        V: TryFrom<u8>,
        A::Item: PrimInt + radium::marker::BitOps,
    {
        loop {
            let store = self.0.load(Ordering::Acquire);
            if store.is_zero() {
                return None;
            }
            let bit = store.trailing_zeros();
            let mask = A::Item::one().unsigned_shl(bit);
            let old = self.0.fetch_and(!mask, Ordering::AcqRel);
            if old & mask != A::Item::zero() {
                let converted = V::try_from(bit as u8);
                debug_assert!(converted.is_ok());
                return Some(match converted {
                    Ok(value) => value,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                });
            }
        }
    }

    #[inline]
    pub fn pop_last(&self) -> Option<V>
    where
        V: TryFrom<u8>,
        A::Item: PrimInt + radium::marker::BitOps,
    {
        loop {
            let store = self.0.load(Ordering::Acquire);
            if store.is_zero() {
                return None;
            }
            let bit = (Self::BITS as u32) - 1 - store.leading_zeros();
            let mask = A::Item::one().unsigned_shl(bit);
            let old = self.0.fetch_and(!mask, Ordering::AcqRel);
            if old & mask != A::Item::zero() {
                let converted = V::try_from(bit as u8);
                debug_assert!(converted.is_ok());
                return Some(match converted {
                    Ok(value) => value,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                });
            }
        }
    }

    #[inline]
    pub fn contains(&self, id: &V) -> bool
    where
        V: Copy + AsPrimitive<usize>,
        A::Item: PrimInt,
    {
        let idx = (*id).as_();
        debug_assert!(
            idx < Self::BITS,
            "index {idx} out of range for capacity {}",
            Self::BITS
        );
        if idx >= Self::BITS {
            return false;
        }
        let store = self.0.load(Ordering::Relaxed);
        let mask = <A::Item as num_traits::One>::one().unsigned_shl(idx as u32);
        store & mask != A::Item::zero()
    }

    #[inline]
    pub fn set(&self, id: V, value: bool)
    where
        V: AsPrimitive<usize>,
        A::Item: PrimInt + radium::marker::BitOps,
    {
        if value {
            self.insert(id);
        } else {
            self.remove(id);
        }
    }

    #[inline]
    pub fn insert(&self, id: V) -> bool
    where
        V: AsPrimitive<usize>,
        A::Item: PrimInt + radium::marker::BitOps,
    {
        let idx = id.as_();
        debug_assert!(
            idx < Self::BITS,
            "index {idx} out of range for capacity {}",
            Self::BITS
        );
        if idx >= Self::BITS {
            return false;
        }
        let mask = <A::Item as num_traits::One>::one().unsigned_shl(idx as u32);
        let old = self.0.fetch_or(mask, Ordering::AcqRel);
        old & mask == A::Item::zero()
    }

    #[inline]
    pub fn remove(&self, id: V) -> bool
    where
        V: AsPrimitive<usize>,
        A::Item: PrimInt + radium::marker::BitOps,
    {
        let idx = id.as_();
        debug_assert!(
            idx < Self::BITS,
            "index {idx} out of range for capacity {}",
            Self::BITS
        );
        if idx >= Self::BITS {
            return false;
        }
        let mask = <A::Item as num_traits::One>::one().unsigned_shl(idx as u32);
        let old = self.0.fetch_and(!mask, Ordering::AcqRel);
        old & mask != A::Item::zero()
    }

    #[inline]
    pub fn toggle(&self, id: V)
    where
        V: AsPrimitive<usize>,
        A::Item: PrimInt + radium::marker::BitOps,
    {
        let idx = id.as_();
        debug_assert!(
            idx < Self::BITS,
            "index {idx} out of range for capacity {}",
            Self::BITS
        );
        if idx >= Self::BITS {
            return;
        }
        let mask = <A::Item as One>::one().unsigned_shl(idx as u32);
        self.0.fetch_xor(mask, Ordering::AcqRel);
    }

    #[inline]
    pub fn clear(&self)
    where
        A::Item: PrimInt,
    {
        self.0.store(A::Item::zero(), Ordering::Release);
    }

    pub fn retain(&self, mut f: impl FnMut(V) -> bool)
    where
        V: TryFrom<u8>,
        A::Item: PrimInt + radium::marker::BitOps,
    {
        let store = self.0.load(Ordering::Relaxed);
        let mut w = store;
        while !w.is_zero() {
            let bit = w.trailing_zeros();
            let mask = A::Item::one().unsigned_shl(bit);
            w = w & !mask;
            let converted = V::try_from(bit as u8);
            debug_assert!(converted.is_ok());
            let value = match converted {
                Ok(v) => v,
                Err(_) => unsafe { core::hint::unreachable_unchecked() },
            };
            if !f(value) {
                self.0.fetch_and(!mask, Ordering::AcqRel);
            }
        }
    }

    #[inline]
    pub fn union(&self, other: A::Item) -> BitSet<A::Item, V>
    where
        A::Item: PrimInt + super::bitset::PrimStore,
    {
        let store = self.0.load(Ordering::Relaxed);
        BitSet(store | other, PhantomData)
    }

    #[inline]
    pub fn difference(&self, other: A::Item) -> BitSet<A::Item, V>
    where
        A::Item: PrimInt + super::bitset::PrimStore,
    {
        let store = self.0.load(Ordering::Relaxed);
        BitSet(store & !other, PhantomData)
    }

    #[inline]
    pub fn iter(&self) -> PrimBitSetIter<A::Item, V>
    where
        A::Item: PrimInt,
    {
        PrimBitSetIter(self.0.load(Ordering::Relaxed), PhantomData)
    }

    #[inline]
    pub fn union_from(&self, other: A::Item)
    where
        A::Item: Copy + radium::marker::BitOps,
    {
        self.0.fetch_or(other, Ordering::AcqRel);
    }

    #[inline]
    pub fn drain(&self) -> PrimBitSetIter<A::Item, V>
    where
        A::Item: Copy + PrimInt + Zero,
    {
        let store = self.0.swap(A::Item::zero(), Ordering::AcqRel);
        PrimBitSetIter(store, PhantomData)
    }

    #[inline]
    pub fn is_subset(&self, other: &Self) -> bool
    where
        A::Item: PrimInt,
    {
        let a = self.0.load(Ordering::Relaxed);
        let b = other.0.load(Ordering::Relaxed);
        a & b == a
    }

    #[inline]
    pub fn is_superset(&self, other: &Self) -> bool
    where
        A::Item: PrimInt,
    {
        other.is_subset(self)
    }

    #[inline]
    pub fn is_disjoint(&self, other: &Self) -> bool
    where
        A::Item: PrimInt,
    {
        let a = self.0.load(Ordering::Relaxed);
        let b = other.0.load(Ordering::Relaxed);
        (a & b).is_zero()
    }
}

impl<A: AtomicPrimStore, V> core::fmt::Binary for AtomicBitSet<A, V>
where
    A::Item: PrimInt + core::fmt::Binary,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Binary::fmt(&self.0.load(Ordering::Relaxed), f)
    }
}

impl<A: AtomicPrimStore, V> core::fmt::Octal for AtomicBitSet<A, V>
where
    A::Item: PrimInt + core::fmt::Octal,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Octal::fmt(&self.0.load(Ordering::Relaxed), f)
    }
}

impl<A: AtomicPrimStore, V> core::fmt::LowerHex for AtomicBitSet<A, V>
where
    A::Item: PrimInt + core::fmt::LowerHex,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(&self.0.load(Ordering::Relaxed), f)
    }
}

impl<A: AtomicPrimStore, V> core::fmt::UpperHex for AtomicBitSet<A, V>
where
    A::Item: PrimInt + core::fmt::UpperHex,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::UpperHex::fmt(&self.0.load(Ordering::Relaxed), f)
    }
}

pub type AtomicArrayBitSet<A, V, const N: usize> = AtomicBitSet<[A; N], V>;

impl<A: Radium, V, const N: usize> core::fmt::Debug for AtomicBitSet<[A; N], V>
where
    A::Item: PrimInt,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut formatter = f.debug_tuple("AtomicBitSet");
        let bits_per = core::mem::size_of::<A>() * 8;
        for (seg_idx, word) in self.0.iter().enumerate() {
            let val = word.load(Ordering::Relaxed);
            for bit in 0..bits_per {
                if val & A::Item::one().unsigned_shl(bit as u32) != A::Item::zero() {
                    formatter.field(&(seg_idx * bits_per + bit));
                }
            }
        }
        formatter.finish()
    }
}

impl<A: Radium, V, const N: usize> Deref for AtomicBitSet<[A; N], V>
where
    A::Item: PrimInt,
{
    type Target = AtomicBitSlice<A, V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        AtomicBitSlice::from_slice_ref(&self.0)
    }
}

impl<A: Radium, V, const N: usize> Default for AtomicBitSet<[A; N], V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Radium, V, const N: usize> AtomicBitSet<[A; N], V> {
    // SAFETY: All-zeros is a valid representation for arrays of atomic integers.
    // Atomics are not Copy, so [A::ZERO; N] doesn't work in const context.
    const ZERO: Self = Self(unsafe { core::mem::zeroed() }, PhantomData);

    #[inline]
    pub const fn new() -> Self {
        Self::ZERO
    }

    #[inline]
    pub fn from_bits(raw: [A; N]) -> Self {
        Self(raw, PhantomData)
    }

    #[inline]
    pub fn from_element(id: V) -> Self
    where
        V: AsPrimitive<usize>,
        A::Item: PrimInt + radium::marker::BitOps,
    {
        let ret = Self::new();
        ret.set(id, true);
        ret
    }

    #[inline]
    pub fn as_bits(&self) -> &[A; N] {
        &self.0
    }

    #[inline]
    pub fn into_bits(self) -> [A; N] {
        self.0
    }

    #[inline]
    pub fn union_from(&self, other: &BitSet<[<A as Radium>::Item; N], V>)
    where
        <A as Radium>::Item: radium::marker::BitOps + Copy + super::bitset::PrimStore,
    {
        for (atomic, value) in self.0.iter().zip(other.as_slice().iter()) {
            atomic.fetch_or(*value, Ordering::AcqRel);
        }
    }

    #[inline]
    pub fn union(
        &self,
        other: &BitSet<[<A as Radium>::Item; N], V>,
    ) -> BitSet<[<A as Radium>::Item; N], V>
    where
        <A as Radium>::Item: Copy + PrimInt + super::bitset::PrimStore,
    {
        let other_bits = &other.0;
        let mut raw = [<A as Radium>::Item::ZERO; N];
        for (i, atomic) in self.0.iter().enumerate() {
            raw[i] = atomic.load(Ordering::Relaxed) | other_bits[i];
        }
        BitSet(raw, PhantomData)
    }

    #[inline]
    pub fn difference(
        &self,
        other: &BitSet<[<A as Radium>::Item; N], V>,
    ) -> BitSet<[<A as Radium>::Item; N], V>
    where
        <A as Radium>::Item: Copy + PrimInt + super::bitset::PrimStore,
    {
        let other_bits = &other.0;
        let mut raw = [<A as Radium>::Item::ZERO; N];
        for (i, atomic) in self.0.iter().enumerate() {
            raw[i] = atomic.load(Ordering::Relaxed) & !other_bits[i];
        }
        BitSet(raw, PhantomData)
    }

    #[inline]
    pub fn drain(&self) -> BitSet<[<A as Radium>::Item; N], V>
    where
        <A as Radium>::Item: Copy + PrimInt + super::bitset::PrimStore,
    {
        let mut raw = [<A as Radium>::Item::ZERO; N];
        for (i, atomic) in self.0.iter().enumerate() {
            raw[i] = atomic.swap(<A as Radium>::Item::zero(), Ordering::AcqRel);
        }
        BitSet(raw, PhantomData)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use core::sync::atomic::{AtomicU32, AtomicU64};
    use proptest::prelude::*;
    use rand::Rng;

    #[test]
    fn test_array_iter_basic() {
        let bs = AtomicBitSet::<[AtomicU64; 2], usize>::new();
        bs.set(0, true);
        bs.set(63, true);
        bs.set(64, true);
        bs.set(127, true);

        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 63, 64, 127]);
    }

    #[test]
    fn test_array_iter_empty() {
        let bs = AtomicBitSet::<[AtomicU64; 2], usize>::new();
        let items: Vec<usize> = bs.iter().collect();
        assert!(items.is_empty());
    }

    #[test]
    fn test_array_iter_cross_segment() {
        let bs = AtomicBitSet::<[AtomicU32; 4], usize>::new();
        bs.set(5, true); // segment 0
        bs.set(33, true); // segment 1
        bs.set(70, true); // segment 2
        bs.set(100, true); // segment 3

        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![5, 33, 70, 100]);
    }

    #[test]
    fn test_array_iter_does_not_consume() {
        let bs = AtomicBitSet::<[AtomicU64; 2], usize>::new();
        bs.set(10, true);
        bs.set(75, true);

        let first: Vec<usize> = bs.iter().collect();
        let second: Vec<usize> = bs.iter().collect();
        assert_eq!(first, second);
    }

    // ── single prim tests ──

    #[test]
    fn test_prim_basic() {
        let bs = AtomicBitSet::<AtomicU64, usize>::new();
        assert!(bs.is_empty());
        assert_eq!(bs.len(), 0);

        // insert returns true on first insert, false on duplicate
        assert!(bs.insert(3));
        assert!(!bs.insert(3));
        assert!(bs.insert(7));
        assert!(bs.insert(42));

        assert!(!bs.is_empty());
        assert_eq!(bs.len(), 3);

        assert!(bs.contains(&3));
        assert!(bs.contains(&7));
        assert!(bs.contains(&42));
        assert!(!bs.contains(&0));
        assert!(!bs.contains(&63));

        // remove returns true when present, false when absent
        assert!(bs.remove(7));
        assert!(!bs.remove(7));
        assert_eq!(bs.len(), 2);
        assert!(!bs.contains(&7));

        // clear
        bs.clear();
        assert!(bs.is_empty());
        assert_eq!(bs.len(), 0);
        assert!(!bs.contains(&3));
    }

    #[test]
    fn test_prim_boundary() {
        let bs = AtomicBitSet::<AtomicU64, usize>::new();

        // bit 0 and bit 63 (max for u64)
        bs.insert(0);
        bs.insert(63);
        assert_eq!(bs.len(), 2);
        assert!(bs.contains(&0));
        assert!(bs.contains(&63));
        assert!(!bs.contains(&1));
        assert!(!bs.contains(&62));

        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 63]);

        bs.remove(63);
        assert!(!bs.contains(&63));
        assert_eq!(bs.len(), 1);

        bs.remove(0);
        assert!(bs.is_empty());
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn test_prim_out_of_range_is_ignored() {
        let bs = AtomicBitSet::<AtomicU64, usize>::new();
        assert!(!bs.contains(&64));
        assert!(!bs.insert(64));
        assert!(!bs.remove(64));
        bs.toggle(64);
        assert!(bs.is_empty());
        assert_eq!(bs.len(), 0);
        assert!(!bs.contains(&0));
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn test_prim_out_of_range_panics_in_debug() {
        let bs = AtomicBitSet::<AtomicU64, usize>::new();
        let _ = bs.contains(&64);
    }

    #[test]
    fn test_prim_set_relations() {
        let a = AtomicBitSet::<AtomicU64, usize>::new();
        a.insert(1);
        a.insert(5);

        let b = AtomicBitSet::<AtomicU64, usize>::new();
        b.insert(1);
        b.insert(5);
        b.insert(10);

        let c = AtomicBitSet::<AtomicU64, usize>::new();
        c.insert(2);
        c.insert(6);

        // a ⊂ b
        assert!(a.is_subset(&b));
        assert!(!b.is_subset(&a));
        assert!(b.is_superset(&a));
        assert!(!a.is_superset(&b));

        // a ∩ c = ∅
        assert!(a.is_disjoint(&c));
        assert!(!a.is_disjoint(&b));
    }

    #[test]
    fn test_prim_drain() {
        let bs = AtomicBitSet::<AtomicU64, usize>::new();
        bs.insert(3);
        bs.insert(10);
        bs.insert(50);

        let drained = bs.drain();
        // original is now empty
        assert!(bs.is_empty());
        assert_eq!(bs.len(), 0);

        // drained contains the values
        let items: Vec<usize> = drained.collect();
        assert_eq!(items, vec![3, 10, 50]);
    }

    #[test]
    fn test_prim_union_from() {
        let bs = AtomicBitSet::<AtomicU64, usize>::new();
        bs.insert(1);
        bs.insert(5);

        // union_from adds bits from a raw value
        let extra: u64 = (1 << 10) | (1 << 5); // bit 10 new, bit 5 already set
        bs.union_from(extra);

        assert!(bs.contains(&1));
        assert!(bs.contains(&5));
        assert!(bs.contains(&10));
        assert_eq!(bs.len(), 3);
    }

    #[test]
    fn test_prim_union_difference() {
        let a = AtomicBitSet::<AtomicU64, usize>::new();
        a.insert(1);
        a.insert(5);

        let other: u64 = (1 << 5) | (1 << 10);

        let union = a.union(other);
        let union_items: Vec<usize> = union.iter().collect();
        assert_eq!(union_items, vec![1, 5, 10]);

        let diff = a.difference(other);
        let diff_items: Vec<usize> = diff.iter().collect();
        assert_eq!(diff_items, vec![1]); // 1 is in a but not in other
    }

    // ── array tests ──

    #[test]
    fn test_array_basic() {
        let bs = AtomicBitSet::<[AtomicU64; 4], usize>::new();
        assert!(bs.is_empty());
        assert_eq!(bs.len(), 0);

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
    fn test_array_boundary() {
        // [AtomicU64; 4] = 256 bits, max index = 255
        let bs = AtomicBitSet::<[AtomicU64; 4], usize>::new();
        bs.insert(0);
        bs.insert(255);
        assert_eq!(bs.len(), 2);
        assert!(bs.contains(&0));
        assert!(bs.contains(&255));
        assert!(!bs.contains(&1));
        assert!(!bs.contains(&254));

        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 255]);

        bs.remove(255);
        assert!(!bs.contains(&255));
        assert_eq!(bs.len(), 1);

        // [AtomicU64; 16] = 1024 bits, max index = 1023
        let bs = AtomicBitSet::<[AtomicU64; 16], usize>::new();
        bs.insert(0);
        bs.insert(1023);
        bs.insert(512);
        assert_eq!(bs.len(), 3);

        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 512, 1023]);

        bs.remove(1023);
        assert!(!bs.contains(&1023));

        // word boundaries
        let bs = AtomicBitSet::<[AtomicU64; 4], usize>::new();
        for i in (0..256).step_by(64) {
            bs.insert(i);
        }
        for i in (63..256).step_by(64) {
            bs.insert(i);
        }
        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 63, 64, 127, 128, 191, 192, 255]);
    }

    #[test]
    fn test_array_set_relations() {
        let a = AtomicBitSet::<[AtomicU64; 4], usize>::new();
        a.insert(1);
        a.insert(65);

        let b = AtomicBitSet::<[AtomicU64; 4], usize>::new();
        b.insert(1);
        b.insert(65);
        b.insert(200);

        let c = AtomicBitSet::<[AtomicU64; 4], usize>::new();
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
    fn test_array_drain() {
        let bs = AtomicBitSet::<[AtomicU64; 4], usize>::new();
        bs.insert(3);
        bs.insert(100);
        bs.insert(200);

        let drained = bs.drain();
        assert!(bs.is_empty());

        let items: Vec<usize> = drained.iter().collect();
        assert_eq!(items, vec![3, 100, 200]);
    }

    #[test]
    fn test_array_union_from() {
        let bs = AtomicBitSet::<[AtomicU64; 4], usize>::new();
        bs.insert(1);
        bs.insert(65);

        let other = BitSet::<[u64; 4], usize>::from_element(200);
        bs.union_from(&other);

        assert!(bs.contains(&1));
        assert!(bs.contains(&65));
        assert!(bs.contains(&200));
        assert_eq!(bs.len(), 3);
    }

    #[test]
    fn test_prim_toggle() {
        let bs = AtomicBitSet::<AtomicU64, usize>::new();
        bs.insert(3);
        bs.toggle(3);
        assert!(!bs.contains(&3));
        bs.toggle(3);
        assert!(bs.contains(&3));
        bs.toggle(10);
        assert!(bs.contains(&10));
    }

    #[test]
    fn test_prim_format_traits() {
        use alloc::format;
        let bs = AtomicBitSet::<AtomicU64, usize>::new();
        bs.insert(1);
        bs.insert(3);
        // bits = 0b1010
        assert_eq!(format!("{bs:b}"), "1010");
        assert_eq!(format!("{bs:o}"), "12");
        assert_eq!(format!("{bs:x}"), "a");
        assert_eq!(format!("{bs:X}"), "A");
    }

    #[test]
    fn test_prim_retain() {
        let bs = AtomicBitSet::<AtomicU64, usize>::new();
        for i in [0, 1, 2, 3, 4, 5] {
            bs.insert(i);
        }
        bs.retain(|v: usize| v >= 3);
        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![3, 4, 5]);
    }

    #[test]
    fn test_array_retain() {
        let bs = AtomicBitSet::<[AtomicU64; 4], usize>::new();
        for i in [1, 3, 5, 64, 66, 100] {
            bs.insert(i);
        }
        bs.retain(|v: usize| v % 2 == 0);
        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![64, 66, 100]);
    }

    #[test]
    fn test_array_toggle() {
        let bs = AtomicBitSet::<[AtomicU64; 4], usize>::new();
        bs.insert(5);
        bs.insert(200);
        bs.toggle(5);
        assert!(!bs.contains(&5));
        bs.toggle(200);
        assert!(!bs.contains(&200));
        bs.toggle(100);
        assert!(bs.contains(&100));
    }

    // ── proptest parity ──

    prop_compose! {
        fn arb_indexes(bits: usize)(size in 0..100usize) -> Vec<usize> {
            let mut rng = rand::thread_rng();
            (0..size).map(|_| rng.gen_range(0..bits)).collect()
        }
    }

    fn assert_set_result<I: PrimInt + core::ops::BitAndAssign<I>>(
        expected: &[usize],
        actual: PrimBitSetIter<I, usize>,
    ) {
        let mut expected = expected.to_vec();
        expected.sort_unstable();
        expected.dedup();
        let actual: Vec<_> = actual.collect();
        assert_eq!(expected, actual);
    }

    proptest! {
        #[test]
        fn flag_set_iter_atomic_32(indexes in arb_indexes(32)) {
            let flags = AtomicBitSet::<AtomicU32, usize>::new();
            for idx in indexes.iter() {
                flags.set(*idx, true);
            }
            assert_set_result(&indexes, flags.drain());
        }

        #[test]
        fn flag_set_iter_atomic_64(indexes in arb_indexes(64)) {
            let flags = AtomicBitSet::<AtomicU64, usize>::new();
            for idx in indexes.iter() {
                flags.set(*idx, true);
            }
            assert_set_result(&indexes, flags.drain());
        }

        #[test]
        fn parity_atomic_prim_64(ops in prop::collection::vec((any::<bool>(), 0..64usize), 0..200)) {
            let atomic = AtomicBitSet::<AtomicU64, usize>::new();
            let mut plain = BitSet::<u64, usize>::new();

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

            for i in 0..64 {
                prop_assert_eq!(atomic.contains(&i), plain.contains(&i), "contains mismatch at {}", i);
            }

            let atomic_bits: Vec<usize> = atomic.iter().collect();
            let plain_bits: Vec<usize> = plain.iter().collect();
            prop_assert_eq!(atomic_bits, plain_bits, "iter mismatch");
        }

        #[test]
        fn parity_atomic_array_256(ops in prop::collection::vec((any::<bool>(), 0..256usize), 0..200)) {
            let atomic = AtomicBitSet::<[AtomicU64; 4], usize>::new();
            let mut plain = BitSet::<[u64; 4], usize>::new();

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

            for i in 0..256 {
                prop_assert_eq!(atomic.contains(&i), plain.contains(&i), "contains mismatch at {}", i);
            }

            let atomic_bits: Vec<usize> = atomic.iter().collect();
            let plain_bits: Vec<usize> = plain.iter().collect();
            prop_assert_eq!(atomic_bits, plain_bits, "iter mismatch");
        }
    }
}
