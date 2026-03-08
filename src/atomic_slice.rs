use core::marker::PhantomData;
use core::ops::BitAndAssign;
use core::sync::atomic::Ordering;
use num_traits::{One, PrimInt, Zero};
use radium::Radium;

use super::bitset::PrimBitSetIter;

/// Unsized shared base for multi-word atomic bitset types.
///
/// `AtomicBitSlice<A, V>` is to atomic bitsets what [`BitSlice<T, V>`](super::BitSlice)
/// is to non-atomic ones: a `#[repr(transparent)]` wrapper around `[A]` that provides
/// common query and mutation methods. Owned types ([`AtomicBitSet<[A; N], V>`](super::AtomicBitSet)
/// and [`AtomicBoxedBitSet<A, V>`](super::AtomicBoxedBitSet)) implement
/// `Deref<Target = AtomicBitSlice<A, V>>`.
///
/// # Atomicity guarantees
///
/// Each individual method (`insert`, `remove`, `contains`, ...) performs atomic
/// operations **per word**. However, the bitset as a whole is **not** a single
/// atomic unit when it spans multiple words. Concurrent modifications to bits
/// within the **same word** are correctly synchronized via `fetch_or` / `fetch_and`
/// with `AcqRel` ordering. Modifications to bits in **different words** are
/// independent atomic operations — there is no cross-word transactional guarantee.
///
/// Read-only methods (`len`, `iter`, `contains`, `is_subset`, …) load each word
/// with `Relaxed` ordering and do not take a consistent snapshot of the entire
/// bitset. If another thread modifies the set concurrently, these methods may
/// observe a mix of old and new state across different words.
#[repr(transparent)]
pub struct AtomicBitSlice<A, V>(PhantomData<V>, [A]);

impl<A, V> AtomicBitSlice<A, V> {
    pub(crate) fn from_slice_ref(s: &[A]) -> &Self {
        // SAFETY: AtomicBitSlice<A, V> is repr(transparent) over [A]
        // (PhantomData<V> is ZST)
        unsafe { &*(s as *const [A] as *const Self) }
    }

    #[inline]
    pub fn as_raw_slice(&self) -> &[A] {
        &self.1
    }
}

impl<A, V> AtomicBitSlice<A, V>
where
    A: Radium,
    A::Item: PrimInt,
{
    const BITS_PER: usize = core::mem::size_of::<A>() * 8;

    #[inline]
    fn index_of(idx: usize) -> (usize, A::Item) {
        (
            idx / Self::BITS_PER,
            <A::Item as num_traits::One>::one().unsigned_shl((idx % Self::BITS_PER) as u32),
        )
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.1.len() * Self::BITS_PER
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.1
            .iter()
            .map(|a| a.load(Ordering::Relaxed).count_ones() as usize)
            .sum()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.1.iter().all(|a| a.load(Ordering::Relaxed).is_zero())
    }

    #[inline]
    pub fn first(&self) -> Option<V>
    where
        V: TryFrom<usize>,
    {
        for (i, a) in self.1.iter().enumerate() {
            let word = a.load(Ordering::Relaxed);
            if !word.is_zero() {
                let bit = word.trailing_zeros() as usize;
                return V::try_from(i * Self::BITS_PER + bit).ok();
            }
        }
        None
    }

    #[inline]
    pub fn last(&self) -> Option<V>
    where
        V: TryFrom<usize>,
    {
        for (i, a) in self.1.iter().enumerate().rev() {
            let word = a.load(Ordering::Relaxed);
            if !word.is_zero() {
                let bit = Self::BITS_PER - 1 - word.leading_zeros() as usize;
                return V::try_from(i * Self::BITS_PER + bit).ok();
            }
        }
        None
    }

    #[inline]
    pub fn pop_first(&self) -> Option<V>
    where
        V: TryFrom<usize>,
        A::Item: radium::marker::BitOps,
    {
        for (i, a) in self.1.iter().enumerate() {
            loop {
                let word = a.load(Ordering::Acquire);
                if word.is_zero() {
                    break;
                }
                let bit = word.trailing_zeros() as usize;
                let mask = A::Item::one().unsigned_shl(bit as u32);
                let old = a.fetch_and(!mask, Ordering::AcqRel);
                if old & mask != A::Item::zero() {
                    return V::try_from(i * Self::BITS_PER + bit).ok();
                }
            }
        }
        None
    }

    #[inline]
    pub fn pop_last(&self) -> Option<V>
    where
        V: TryFrom<usize>,
        A::Item: radium::marker::BitOps,
    {
        for (i, a) in self.1.iter().enumerate().rev() {
            loop {
                let word = a.load(Ordering::Acquire);
                if word.is_zero() {
                    break;
                }
                let bit = Self::BITS_PER - 1 - word.leading_zeros() as usize;
                let mask = A::Item::one().unsigned_shl(bit as u32);
                let old = a.fetch_and(!mask, Ordering::AcqRel);
                if old & mask != A::Item::zero() {
                    return V::try_from(i * Self::BITS_PER + bit).ok();
                }
            }
        }
        None
    }

    #[inline]
    pub fn contains(&self, id: &V) -> bool
    where
        V: Copy + num_traits::AsPrimitive<usize>,
    {
        let idx = (*id).as_();
        let (seg, mask) = Self::index_of(idx);
        if seg >= self.1.len() {
            return false;
        }
        // SAFETY: seg < self.1.len() checked above.
        let a = unsafe { self.1.get_unchecked(seg) };
        a.load(Ordering::Relaxed) & mask != A::Item::zero()
    }

    #[inline]
    pub fn insert(&self, id: V) -> bool
    where
        V: num_traits::AsPrimitive<usize>,
        A::Item: radium::marker::BitOps,
    {
        let idx = id.as_();
        let (seg, mask) = Self::index_of(idx);
        if seg >= self.1.len() {
            return false;
        }
        // SAFETY: seg < self.1.len() checked above.
        let a = unsafe { self.1.get_unchecked(seg) };
        let old = a.fetch_or(mask, Ordering::AcqRel);
        old & mask == A::Item::zero()
    }

    #[inline]
    pub fn remove(&self, id: V) -> bool
    where
        V: num_traits::AsPrimitive<usize>,
        A::Item: radium::marker::BitOps,
    {
        let idx = id.as_();
        let (seg, mask) = Self::index_of(idx);
        if seg >= self.1.len() {
            return false;
        }
        // SAFETY: seg < self.1.len() checked above.
        let a = unsafe { self.1.get_unchecked(seg) };
        let old = a.fetch_and(!mask, Ordering::AcqRel);
        old & mask != A::Item::zero()
    }

    #[inline]
    pub fn set(&self, id: V, value: bool)
    where
        V: num_traits::AsPrimitive<usize>,
        A::Item: radium::marker::BitOps,
    {
        if value {
            self.insert(id);
        } else {
            self.remove(id);
        }
    }

    #[inline]
    pub fn toggle(&self, id: V)
    where
        V: num_traits::AsPrimitive<usize>,
        A::Item: radium::marker::BitOps,
    {
        let idx = id.as_();
        let (seg, mask) = Self::index_of(idx);
        if seg >= self.1.len() {
            return;
        }
        // SAFETY: seg < self.1.len() checked above.
        let a = unsafe { self.1.get_unchecked(seg) };
        a.fetch_xor(mask, Ordering::AcqRel);
    }

    #[inline]
    pub fn clear(&self) {
        for atomic in self.1.iter() {
            atomic.store(A::Item::zero(), Ordering::Release);
        }
    }

    pub fn retain(&self, mut f: impl FnMut(V) -> bool)
    where
        V: TryFrom<usize>,
        A::Item: radium::marker::BitOps,
    {
        for (i, a) in self.1.iter().enumerate() {
            let word = a.load(Ordering::Relaxed);
            let mut w = word;
            while !w.is_zero() {
                let bit = w.trailing_zeros() as usize;
                let mask = A::Item::one().unsigned_shl(bit as u32);
                w = w & !mask;
                let idx = i * Self::BITS_PER + bit;
                debug_assert!(V::try_from(idx).is_ok());
                let value = match V::try_from(idx) {
                    Ok(v) => v,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                };
                if !f(value) {
                    a.fetch_and(!mask, Ordering::AcqRel);
                }
            }
        }
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = V> + '_
    where
        A::Item: BitAndAssign,
        V: TryFrom<usize>,
    {
        self.1.iter().enumerate().flat_map(move |(i, a)| {
            let bits = a.load(Ordering::Relaxed);
            let offset = i * Self::BITS_PER;
            PrimBitSetIter::<A::Item, usize>(bits, PhantomData).map(move |pos| {
                let idx = offset + pos;
                debug_assert!(V::try_from(idx).is_ok());
                match V::try_from(idx) {
                    Ok(v) => v,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                }
            })
        })
    }

    #[inline]
    pub fn is_subset(&self, other: &Self) -> bool {
        let min = self.1.len().min(other.1.len());
        self.1[..min]
            .iter()
            .zip(other.1[..min].iter())
            .all(|(a, b)| {
                let va = a.load(Ordering::Relaxed);
                let vb = b.load(Ordering::Relaxed);
                va & vb == va
            })
            && self.1[min..]
                .iter()
                .all(|a| a.load(Ordering::Relaxed).is_zero())
    }

    #[inline]
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    #[inline]
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.1.iter().zip(other.1.iter()).all(|(a, b)| {
            let va = a.load(Ordering::Relaxed);
            let vb = b.load(Ordering::Relaxed);
            (va & vb).is_zero()
        })
    }

    fn word_op_iter<'a>(
        a: &'a [A],
        b: &'a [A],
        len: usize,
        op: impl Fn(A::Item, A::Item) -> A::Item + 'a,
    ) -> impl Iterator<Item = V> + 'a
    where
        A::Item: BitAndAssign,
        V: TryFrom<usize>,
    {
        let bits_per = Self::BITS_PER;
        (0..len).flat_map(move |i| {
            let w_a = a
                .get(i)
                .map(|a| a.load(Ordering::Relaxed))
                .unwrap_or(A::Item::zero());
            let w_b = b
                .get(i)
                .map(|a| a.load(Ordering::Relaxed))
                .unwrap_or(A::Item::zero());
            let combined = op(w_a, w_b);
            let offset = i * bits_per;
            PrimBitSetIter::<A::Item, usize>(combined, PhantomData).map(move |pos| {
                let idx = offset + pos;
                debug_assert!(V::try_from(idx).is_ok());
                match V::try_from(idx) {
                    Ok(v) => v,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                }
            })
        })
    }

    #[inline]
    pub fn difference<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = V> + 'a
    where
        A::Item: BitAndAssign,
        V: TryFrom<usize>,
    {
        Self::word_op_iter(&self.1, &other.1, self.1.len(), |a, b| a & !b)
    }

    #[inline]
    pub fn intersection<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = V> + 'a
    where
        A::Item: BitAndAssign,
        V: TryFrom<usize>,
    {
        Self::word_op_iter(
            &self.1,
            &other.1,
            self.1.len().min(other.1.len()),
            |a, b| a & b,
        )
    }

    #[inline]
    pub fn union<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = V> + 'a
    where
        A::Item: BitAndAssign,
        V: TryFrom<usize>,
    {
        Self::word_op_iter(
            &self.1,
            &other.1,
            self.1.len().max(other.1.len()),
            |a, b| a | b,
        )
    }

    #[inline]
    pub fn symmetric_difference<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = V> + 'a
    where
        A::Item: BitAndAssign,
        V: TryFrom<usize>,
    {
        Self::word_op_iter(
            &self.1,
            &other.1,
            self.1.len().max(other.1.len()),
            |a, b| a ^ b,
        )
    }

    pub fn append(&self, other: &Self)
    where
        A::Item: radium::marker::BitOps + Copy,
    {
        let min = self.1.len().min(other.1.len());
        for i in 0..min {
            let val = other.1[i].swap(A::Item::zero(), Ordering::AcqRel);
            self.1[i].fetch_or(val, Ordering::AcqRel);
        }
        // Clear remaining words in other beyond self's range
        for a in &other.1[min..] {
            a.store(A::Item::zero(), Ordering::Release);
        }
    }

    #[inline]
    pub fn union_from(&self, other: &[A::Item])
    where
        A::Item: radium::marker::BitOps + Copy,
    {
        for (atomic, &value) in self.1.iter().zip(other.iter()) {
            atomic.fetch_or(value, Ordering::AcqRel);
        }
    }
}

impl<A, V> core::fmt::Debug for AtomicBitSlice<A, V>
where
    A: Radium,
    A::Item: PrimInt + BitAndAssign,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let bits_per = core::mem::size_of::<A>() * 8;
        f.write_str("{")?;
        let mut first = true;
        for (i, a) in self.1.iter().enumerate() {
            let word = a.load(Ordering::Relaxed);
            let offset = i * bits_per;
            for pos in PrimBitSetIter::<A::Item, usize>(word, PhantomData) {
                if !first {
                    f.write_str(", ")?;
                }
                first = false;
                write!(f, "{}", offset + pos)?;
            }
        }
        f.write_str("}")
    }
}
