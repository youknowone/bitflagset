use core::hash::{Hash, Hasher};
use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::ops::BitAndAssign;
use num_traits::{AsPrimitive, PrimInt};

use super::bitset::PrimBitSetIter;

/// Iterator over set bit positions in a word slice.
///
/// The storage `S` can be `&[T]`, `[T; N]`, `Box<[T]>`, etc.
pub struct WordSetIter<S, T: PrimInt, V> {
    store: S,
    word_idx: usize,
    current: PrimBitSetIter<T, usize>,
    _marker: PhantomData<V>,
}

impl<S: AsRef<[T]>, T: PrimInt + BitAndAssign, V> WordSetIter<S, T, V> {
    #[inline]
    pub(crate) fn new(store: S) -> Self {
        Self {
            store,
            word_idx: 0,
            current: PrimBitSetIter::empty(),
            _marker: PhantomData,
        }
    }

    #[inline]
    fn remaining_len(&self) -> usize {
        self.current.len()
            + self.store.as_ref()[self.word_idx..]
                .iter()
                .map(|w| w.count_ones() as usize)
                .sum::<usize>()
    }
}

impl<S: AsRef<[T]>, T: PrimInt + BitAndAssign, V: TryFrom<usize>> Iterator
    for WordSetIter<S, T, V>
{
    type Item = V;

    fn next(&mut self) -> Option<V> {
        let words = self.store.as_ref();
        let bits_per = core::mem::size_of::<T>() * 8;
        loop {
            if let Some(pos) = self.current.next() {
                let idx = (self.word_idx - 1) * bits_per + pos;
                let converted = V::try_from(idx);
                debug_assert!(converted.is_ok());
                match converted {
                    Ok(value) => return Some(value),
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                }
            }
            if self.word_idx >= words.len() {
                return None;
            }
            self.current = PrimBitSetIter::from_raw(words[self.word_idx]);
            self.word_idx += 1;
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.remaining_len();
        (len, Some(len))
    }

    #[inline]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.remaining_len()
    }
}

impl<S: AsRef<[T]>, T: PrimInt + BitAndAssign, V: TryFrom<usize>> ExactSizeIterator
    for WordSetIter<S, T, V>
{
    #[inline]
    fn len(&self) -> usize {
        self.remaining_len()
    }
}

impl<S: AsRef<[T]>, T: PrimInt + BitAndAssign, V: TryFrom<usize>> FusedIterator
    for WordSetIter<S, T, V>
{
}

/// Iterator over set bit positions in a `BitSlice`.
pub type BitSliceIter<'a, T, V> = WordSetIter<&'a [T], T, V>;

/// Draining iterator over set bit positions in a `BitSlice`.
///
/// Each word is consumed and zeroed in-place as iteration advances.
/// Dropping the iterator clears any remaining words.
pub struct Drain<'a, T: PrimInt, V> {
    words: &'a mut [T],
    word_idx: usize,
    current: PrimBitSetIter<T, usize>,
    _marker: PhantomData<V>,
}

impl<T: PrimInt + BitAndAssign, V> Drain<'_, T, V> {
    #[inline]
    fn remaining_len(&self) -> usize {
        self.current.len()
            + self.words[self.word_idx..]
                .iter()
                .map(|w| w.count_ones() as usize)
                .sum::<usize>()
    }
}

impl<T: PrimInt + BitAndAssign, V: TryFrom<usize>> Iterator for Drain<'_, T, V> {
    type Item = V;

    fn next(&mut self) -> Option<V> {
        let bits_per = core::mem::size_of::<T>() * 8;
        loop {
            if let Some(pos) = self.current.next() {
                let idx = (self.word_idx - 1) * bits_per + pos;
                let converted = V::try_from(idx);
                debug_assert!(converted.is_ok());
                match converted {
                    Ok(value) => return Some(value),
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                }
            }
            if self.word_idx >= self.words.len() {
                return None;
            }
            self.current = PrimBitSetIter::from_raw(self.words[self.word_idx]);
            self.words[self.word_idx] = T::zero();
            self.word_idx += 1;
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.remaining_len();
        (len, Some(len))
    }

    #[inline]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.remaining_len()
    }
}

impl<T: PrimInt + BitAndAssign, V: TryFrom<usize>> ExactSizeIterator for Drain<'_, T, V> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining_len()
    }
}

impl<T: PrimInt + BitAndAssign, V: TryFrom<usize>> FusedIterator for Drain<'_, T, V> {}

impl<T: PrimInt, V> Drop for Drain<'_, T, V> {
    fn drop(&mut self) {
        // Clear any words not yet consumed
        for w in &mut self.words[self.word_idx..] {
            *w = T::zero();
        }
    }
}

/// Unsized shared base for all bitset types. Wraps a raw `[T]` primitive slice.
///
/// All operations use direct primitive bit manipulation (count_ones, bit masking, etc.),
/// not bitvec's generic algorithms.
///
/// Owned types (`BitSet`, `BoxedBitSet`) implement `Deref<Target = BitSlice<T, V>>`
/// so common methods are defined here once.
#[repr(transparent)]
pub struct BitSlice<T, V>(PhantomData<V>, [T]);

impl<T, V> BitSlice<T, V> {
    pub(crate) fn from_slice_ref(s: &[T]) -> &Self {
        // SAFETY: BitSlice<T, V> is repr(transparent) over [T]
        // (PhantomData<V> is ZST)
        unsafe { &*(s as *const [T] as *const Self) }
    }

    pub(crate) fn from_slice_mut(s: &mut [T]) -> &mut Self {
        // SAFETY: same layout guarantee
        unsafe { &mut *(s as *mut [T] as *mut Self) }
    }
}

impl<T: PrimInt, V> BitSlice<T, V> {
    const BITS_PER: usize = core::mem::size_of::<T>() * 8;

    #[inline]
    fn index_of(idx: usize) -> (usize, T) {
        (
            idx / Self::BITS_PER,
            T::one().unsigned_shl((idx % Self::BITS_PER) as u32),
        )
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.1.len() * Self::BITS_PER
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.1.iter().map(|w| w.count_ones() as usize).sum()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.1.iter().all(|w| w.is_zero())
    }

    #[inline]
    pub fn first(&self) -> Option<V>
    where
        V: TryFrom<usize>,
    {
        for (i, &word) in self.1.iter().enumerate() {
            if !word.is_zero() {
                let bit = word.trailing_zeros() as usize;
                let idx = i * Self::BITS_PER + bit;
                let converted = V::try_from(idx);
                debug_assert!(converted.is_ok());
                return Some(match converted {
                    Ok(value) => value,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                });
            }
        }
        None
    }

    #[inline]
    pub fn last(&self) -> Option<V>
    where
        V: TryFrom<usize>,
    {
        for (i, &word) in self.1.iter().enumerate().rev() {
            if !word.is_zero() {
                let bit = Self::BITS_PER - 1 - word.leading_zeros() as usize;
                let idx = i * Self::BITS_PER + bit;
                let converted = V::try_from(idx);
                debug_assert!(converted.is_ok());
                return Some(match converted {
                    Ok(value) => value,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                });
            }
        }
        None
    }

    #[inline]
    pub fn pop_first(&mut self) -> Option<V>
    where
        V: TryFrom<usize>,
    {
        for (i, word) in self.1.iter_mut().enumerate() {
            if !word.is_zero() {
                let bit = word.trailing_zeros() as usize;
                let mask = T::one().unsigned_shl(bit as u32);
                *word = *word & !mask;
                let idx = i * Self::BITS_PER + bit;
                let converted = V::try_from(idx);
                debug_assert!(converted.is_ok());
                return Some(match converted {
                    Ok(value) => value,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                });
            }
        }
        None
    }

    #[inline]
    pub fn pop_last(&mut self) -> Option<V>
    where
        V: TryFrom<usize>,
    {
        for (i, word) in self.1.iter_mut().enumerate().rev() {
            if !word.is_zero() {
                let bit = Self::BITS_PER - 1 - word.leading_zeros() as usize;
                let mask = T::one().unsigned_shl(bit as u32);
                *word = *word & !mask;
                let idx = i * Self::BITS_PER + bit;
                let converted = V::try_from(idx);
                debug_assert!(converted.is_ok());
                return Some(match converted {
                    Ok(value) => value,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                });
            }
        }
        None
    }

    #[inline]
    pub fn contains(&self, id: &V) -> bool
    where
        V: Copy + AsPrimitive<usize>,
    {
        let idx = (*id).as_();
        debug_assert!(
            idx < self.capacity(),
            "index {idx} out of range for capacity {}",
            self.capacity()
        );
        let (seg, mask) = Self::index_of(idx);
        self.1.get(seg).is_some_and(|w| *w & mask != T::zero())
    }

    #[inline]
    pub fn set(&mut self, id: V, value: bool)
    where
        V: AsPrimitive<usize>,
    {
        let idx = id.as_();
        debug_assert!(
            idx < self.capacity(),
            "index {idx} out of range for capacity {}",
            self.capacity()
        );
        let (seg, mask) = Self::index_of(idx);
        if let Some(word) = self.1.get_mut(seg) {
            if value {
                *word = *word | mask;
            } else {
                *word = *word & !mask;
            }
        }
    }

    #[inline]
    pub fn insert(&mut self, id: V) -> bool
    where
        V: AsPrimitive<usize>,
    {
        let idx = id.as_();
        debug_assert!(
            idx < self.capacity(),
            "index {idx} out of range for capacity {}",
            self.capacity()
        );
        let (seg, mask) = Self::index_of(idx);
        let Some(word) = self.1.get_mut(seg) else {
            return false;
        };
        let was_absent = *word & mask == T::zero();
        *word = *word | mask;
        was_absent
    }

    #[inline]
    pub fn remove(&mut self, id: V) -> bool
    where
        V: AsPrimitive<usize>,
    {
        let idx = id.as_();
        debug_assert!(
            idx < self.capacity(),
            "index {idx} out of range for capacity {}",
            self.capacity()
        );
        let (seg, mask) = Self::index_of(idx);
        let Some(word) = self.1.get_mut(seg) else {
            return false;
        };
        let was_present = *word & mask != T::zero();
        *word = *word & !mask;
        was_present
    }

    #[inline]
    pub fn toggle(&mut self, id: V)
    where
        V: AsPrimitive<usize>,
    {
        let idx = id.as_();
        debug_assert!(
            idx < self.capacity(),
            "index {idx} out of range for capacity {}",
            self.capacity()
        );
        let (seg, mask) = Self::index_of(idx);
        if let Some(word) = self.1.get_mut(seg) {
            *word = *word ^ mask;
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.1.fill(T::zero());
    }

    #[inline]
    pub fn drain(&mut self) -> Drain<'_, T, V>
    where
        T: BitAndAssign,
        V: TryFrom<usize>,
    {
        Drain {
            words: &mut self.1,
            word_idx: 0,
            current: PrimBitSetIter::empty(),
            _marker: PhantomData,
        }
    }

    pub fn retain(&mut self, mut f: impl FnMut(V) -> bool)
    where
        V: TryFrom<usize>,
    {
        for (i, word) in self.1.iter_mut().enumerate() {
            let mut w = *word;
            while !w.is_zero() {
                let bit = w.trailing_zeros() as usize;
                let mask = T::one().unsigned_shl(bit as u32);
                w = w & !mask;
                let idx = i * Self::BITS_PER + bit;
                let converted = V::try_from(idx);
                debug_assert!(converted.is_ok());
                let value = match converted {
                    Ok(v) => v,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                };
                if !f(value) {
                    *word = *word & !mask;
                }
            }
        }
    }

    pub fn append(&mut self, other: &mut Self) {
        let min = self.1.len().min(other.1.len());
        for i in 0..min {
            self.1[i] = self.1[i] | other.1[i];
            other.1[i] = T::zero();
        }
    }

    #[inline]
    pub fn iter(&self) -> BitSliceIter<'_, T, V>
    where
        T: BitAndAssign,
        V: TryFrom<usize>,
    {
        WordSetIter::new(&self.1)
    }

    #[inline]
    pub fn is_subset(&self, other: &Self) -> bool {
        let min = self.1.len().min(other.1.len());
        self.1[..min]
            .iter()
            .zip(other.1[..min].iter())
            .all(|(a, b)| *a & *b == *a)
            && self.1[min..].iter().all(|w| w.is_zero())
    }

    #[inline]
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    #[inline]
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.1
            .iter()
            .zip(other.1.iter())
            .all(|(a, b)| (*a & *b).is_zero())
    }

    fn word_op_iter<'a>(
        a: &'a [T],
        b: &'a [T],
        len: usize,
        op: impl Fn(T, T) -> T + 'a,
    ) -> impl Iterator<Item = V> + 'a
    where
        T: BitAndAssign,
        V: TryFrom<usize>,
    {
        let bits_per = Self::BITS_PER;
        (0..len).flat_map(move |i| {
            let w_a = a.get(i).copied().unwrap_or(T::zero());
            let w_b = b.get(i).copied().unwrap_or(T::zero());
            let combined = op(w_a, w_b);
            let offset = i * bits_per;
            PrimBitSetIter::<T, usize>(combined, PhantomData).map(move |pos| {
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
        T: BitAndAssign,
        V: TryFrom<usize>,
    {
        Self::word_op_iter(&self.1, &other.1, self.1.len(), |a, b| a & !b)
    }

    #[inline]
    pub fn intersection<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = V> + 'a
    where
        T: BitAndAssign,
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
        T: BitAndAssign,
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
        T: BitAndAssign,
        V: TryFrom<usize>,
    {
        Self::word_op_iter(
            &self.1,
            &other.1,
            self.1.len().max(other.1.len()),
            |a, b| a ^ b,
        )
    }

    // bitvec interop

    #[cfg(feature = "bitvec")]
    #[inline]
    pub fn as_bitvec_slice(&self) -> &bitvec::slice::BitSlice<T, bitvec::order::Lsb0>
    where
        T: bitvec::store::BitStore,
    {
        bitvec::slice::BitSlice::from_slice(&self.1)
    }

    #[cfg(feature = "bitvec")]
    #[inline]
    pub fn as_mut_bitvec_slice(&mut self) -> &mut bitvec::slice::BitSlice<T, bitvec::order::Lsb0>
    where
        T: bitvec::store::BitStore,
    {
        bitvec::slice::BitSlice::from_slice_mut(&mut self.1)
    }

    /// Raw word slice accessor.
    #[inline]
    pub fn raw_words(&self) -> &[T] {
        &self.1
    }
}

impl<'a, T: PrimInt + BitAndAssign, V: TryFrom<usize>> IntoIterator for &'a BitSlice<T, V> {
    type Item = V;
    type IntoIter = BitSliceIter<'a, T, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: PrimInt, V> PartialEq for BitSlice<T, V> {
    fn eq(&self, other: &Self) -> bool {
        let min = self.1.len().min(other.1.len());
        self.1[..min] == other.1[..min]
            && self.1[min..].iter().all(|w| w.is_zero())
            && other.1[min..].iter().all(|w| w.is_zero())
    }
}

impl<T: PrimInt, V> Eq for BitSlice<T, V> {}

impl<T: PrimInt + Hash, V> Hash for BitSlice<T, V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash only up to the last non-zero word for length-independent hashing
        let effective_len = self
            .1
            .iter()
            .rposition(|w| !w.is_zero())
            .map_or(0, |i| i + 1);
        for w in &self.1[..effective_len] {
            w.hash(state);
        }
    }
}

impl<T: PrimInt + BitAndAssign, V> core::fmt::Debug for BitSlice<T, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let bits_per = core::mem::size_of::<T>() * 8;
        f.write_str("{")?;
        let mut first = true;
        for (i, &word) in self.1.iter().enumerate() {
            let offset = i * bits_per;
            for pos in PrimBitSetIter::<T, usize>(word, PhantomData) {
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
