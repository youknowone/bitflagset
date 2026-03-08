use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use num_traits::{AsPrimitive, PrimInt, Zero};
use ref_cast::RefCast;

use super::slice::BitSlice;

#[derive(Clone)]
pub struct PrimBitSetIter<I: PrimInt, V>(pub I, pub PhantomData<V>);

impl<I: PrimInt, V> PrimBitSetIter<I, V> {
    #[inline]
    pub const fn from_raw(raw: I) -> Self {
        Self(raw, PhantomData)
    }
    #[inline]
    pub fn empty() -> Self {
        Self(I::zero(), PhantomData)
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_zero()
    }
    #[inline]
    pub fn into_bits(&self) -> I {
        self.0
    }
}

impl<I, V> core::iter::Iterator for PrimBitSetIter<I, V>
where
    I: PrimInt + core::ops::BitAndAssign<I>,
    V: TryFrom<u8>,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_zero() {
            return None;
        }
        let idx = self.0.trailing_zeros();
        self.0.bitand_assign(!(I::one().unsigned_shl(idx)));

        let converted = V::try_from(idx as u8);
        debug_assert!(converted.is_ok());
        Some(converted.unwrap_or_else(|_| unsafe {
            // SAFETY: bit index always fits in u8 (max 63 for u64)
            core::hint::unreachable_unchecked()
        }))
    }
}

impl<I, V> core::iter::ExactSizeIterator for PrimBitSetIter<I, V>
where
    I: PrimInt + core::ops::BitAndAssign<I>,
    V: TryFrom<u8>,
{
    #[inline]
    fn len(&self) -> usize {
        self.0.count_ones() as usize
    }
}

impl<I: PrimInt + core::ops::BitAndAssign<I>, V: TryFrom<u8>> core::iter::FusedIterator
    for PrimBitSetIter<I, V>
{
}

// Sealed marker: only applies to single primitive stores.
// Prevents coherence conflicts with [T; N].
// Public but cannot be implemented externally (sealed).
mod sealed {
    pub trait PrimStore: num_traits::PrimInt {
        const ZERO: Self;
    }
    impl PrimStore for u8 {
        const ZERO: Self = 0;
    }
    impl PrimStore for u16 {
        const ZERO: Self = 0;
    }
    impl PrimStore for u32 {
        const ZERO: Self = 0;
    }
    impl PrimStore for u64 {
        const ZERO: Self = 0;
    }
    impl PrimStore for u128 {
        const ZERO: Self = 0;
    }
    impl PrimStore for usize {
        const ZERO: Self = 0;
    }
}
pub use sealed::PrimStore;

#[repr(transparent)]
#[derive(Clone, Copy, RefCast)]
pub struct BitSet<A, V>(pub(crate) A, #[trivial] pub(crate) PhantomData<V>);

impl<A: PrimStore, V> Deref for BitSet<A, V> {
    type Target = BitSlice<A, V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        BitSlice::from_slice_ref(core::slice::from_ref(&self.0))
    }
}

impl<A: PrimStore, V> DerefMut for BitSet<A, V> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        BitSlice::from_slice_mut(core::slice::from_mut(&mut self.0))
    }
}

impl<A: PrimStore + PartialEq, V> PartialEq for BitSet<A, V> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<A: PrimStore + Eq, V> Eq for BitSet<A, V> {}

impl<A: PrimStore + Ord, V> PartialOrd for BitSet<A, V> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<A: PrimStore + Ord, V> Ord for BitSet<A, V> {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<A: PrimStore + Hash, V> Hash for BitSet<A, V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<A: PrimStore, V> core::fmt::Debug for BitSet<A, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut formatter = f.debug_tuple("BitSet");
        let bits = core::mem::size_of::<A>() * 8;
        for idx in 0..bits {
            if self.0 & A::one().unsigned_shl(idx as u32) != A::zero() {
                formatter.field(&idx);
            }
        }
        formatter.finish()
    }
}

impl<A: PrimStore, V> BitSet<A, V> {
    const ZERO: Self = Self(A::ZERO, PhantomData);
    const BITS: usize = core::mem::size_of::<A>() * 8;

    #[inline]
    pub const fn new() -> Self {
        Self::ZERO
    }

    #[inline]
    pub fn from_element(elem: V) -> Self
    where
        V: AsPrimitive<usize>,
    {
        let mut ret = Self::new();
        ret.set(elem, true);
        ret
    }

    #[inline]
    pub fn into_bits(self) -> A {
        self.0
    }

    #[inline]
    pub fn union(&self, other: &A) -> BitSet<A, V>
    where
        A: Copy + core::ops::BitOr<Output = A>,
    {
        BitSet(self.0 | *other, PhantomData)
    }

    #[inline]
    pub fn difference(&self, other: &A) -> BitSet<A, V>
    where
        A: Copy + core::ops::BitAnd<Output = A> + core::ops::Not<Output = A>,
    {
        BitSet(self.0 & !*other, PhantomData)
    }

    /// Snapshot iterator over set bit positions.
    /// Copies the raw bits, so the bitset can be mutated while iterating.
    #[inline]
    pub fn iter(&self) -> PrimBitSetIter<A, V>
    where
        A: PrimInt,
    {
        PrimBitSetIter(self.0, PhantomData)
    }

    #[inline]
    pub fn load_store(&self) -> A
    where
        A: Copy,
    {
        self.0
    }

    #[inline]
    pub fn swap_store(&mut self, store: &mut A) {
        core::mem::swap(&mut self.0, store);
    }

    #[inline]
    pub fn mut_store(&mut self, f: impl Fn(&mut A)) {
        f(&mut self.0);
    }

    #[inline]
    pub fn drain(&mut self) -> PrimBitSetIter<A, V>
    where
        A: PrimInt + Zero,
    {
        let mut store = A::zero();
        self.swap_store(&mut store);
        PrimBitSetIter(store, PhantomData)
    }

    #[inline]
    pub fn union_from(&mut self, other: A)
    where
        A: Copy + core::ops::BitOrAssign<A>,
    {
        self.0 |= other;
    }

    #[inline]
    pub fn set(&mut self, id: V, value: bool)
    where
        V: AsPrimitive<usize>,
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
        let bit = A::one().unsigned_shl(idx as u32);
        if value {
            self.0 = self.0 | bit;
        } else {
            self.0 = self.0 & !bit;
        }
    }

    #[inline]
    pub fn insert(&mut self, id: V) -> bool
    where
        V: AsPrimitive<usize>,
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
        let bit = A::one().unsigned_shl(idx as u32);
        let was_absent = self.0 & bit == A::zero();
        self.0 = self.0 | bit;
        was_absent
    }

    #[inline]
    pub fn remove(&mut self, id: V) -> bool
    where
        V: AsPrimitive<usize>,
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
        let bit = A::one().unsigned_shl(idx as u32);
        let was_present = self.0 & bit != A::zero();
        self.0 = self.0 & !bit;
        was_present
    }

    #[inline]
    pub fn toggle(&mut self, id: V)
    where
        V: AsPrimitive<usize>,
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
        self.0 = self.0 ^ A::one().unsigned_shl(idx as u32);
    }

    #[cfg(feature = "bitvec")]
    #[inline]
    pub fn as_bitvec_array(&self) -> &bitvec::array::BitArray<A>
    where
        A: bitvec::store::BitStore + bitvec::view::BitViewSized,
    {
        // SAFETY: BitArray<A> is #[repr(transparent)] over A
        unsafe { &*(&self.0 as *const A as *const bitvec::array::BitArray<A>) }
    }

    #[cfg(feature = "bitvec")]
    #[inline]
    pub fn into_bitvec_array(self) -> bitvec::array::BitArray<A>
    where
        A: bitvec::store::BitStore + bitvec::view::BitViewSized,
    {
        bitvec::array::BitArray::new(self.0)
    }
}

impl<A: PrimStore, V> core::iter::IntoIterator for BitSet<A, V>
where
    A: PrimInt + core::ops::BitAndAssign<A>,
    V: TryFrom<u8>,
{
    type Item = V;
    type IntoIter = PrimBitSetIter<A, V>;

    fn into_iter(self) -> Self::IntoIter {
        PrimBitSetIter(self.0, PhantomData)
    }
}

impl<'a, A: PrimStore + core::ops::BitAndAssign, V: TryFrom<usize>> IntoIterator
    for &'a BitSet<A, V>
{
    type Item = V;
    type IntoIter = super::slice::BitSliceIter<'a, A, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        (**self).iter()
    }
}

impl<A: PrimStore, V> Default for BitSet<A, V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Copy + PrimStore, V: AsPrimitive<usize>> FromIterator<V> for BitSet<A, V> {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        let mut ret = Self::new();

        for item in iter {
            ret.set(item, true);
        }

        ret
    }
}

impl<A: Copy + PrimStore, V: AsPrimitive<usize>> core::iter::Extend<V> for BitSet<A, V> {
    fn extend<I: IntoIterator<Item = V>>(&mut self, iter: I) {
        for item in iter {
            self.set(item, true);
        }
    }
}

impl<A: PrimStore + core::ops::BitOrAssign + Copy, V> FromIterator<BitSet<A, V>> for BitSet<A, V> {
    fn from_iter<I: IntoIterator<Item = Self>>(iter: I) -> Self {
        let mut ret = Self::new();
        for bs in iter {
            ret.0 |= bs.0;
        }
        ret
    }
}

impl<A: PrimStore + core::ops::BitOrAssign + Copy, V> core::iter::Extend<BitSet<A, V>>
    for BitSet<A, V>
{
    fn extend<I: IntoIterator<Item = Self>>(&mut self, iter: I) {
        for bs in iter {
            self.0 |= bs.0;
        }
    }
}

impl<A: PrimStore + core::fmt::Binary, V> core::fmt::Binary for BitSet<A, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Binary::fmt(&self.0, f)
    }
}

impl<A: PrimStore + core::fmt::Octal, V> core::fmt::Octal for BitSet<A, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Octal::fmt(&self.0, f)
    }
}

impl<A: PrimStore + core::fmt::LowerHex, V> core::fmt::LowerHex for BitSet<A, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(&self.0, f)
    }
}

impl<A: PrimStore + core::fmt::UpperHex, V> core::fmt::UpperHex for BitSet<A, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::UpperHex::fmt(&self.0, f)
    }
}

#[cfg(feature = "bitvec")]
impl<A, V> From<bitvec::array::BitArray<A>> for BitSet<A, V>
where
    A: PrimStore + bitvec::store::BitStore + bitvec::view::BitViewSized,
{
    #[inline]
    fn from(arr: bitvec::array::BitArray<A>) -> Self {
        Self(arr.into_inner(), PhantomData)
    }
}

#[cfg(feature = "bitvec")]
impl<A, V> From<BitSet<A, V>> for bitvec::array::BitArray<A>
where
    A: PrimStore + bitvec::store::BitStore + bitvec::view::BitViewSized,
{
    #[inline]
    fn from(bs: BitSet<A, V>) -> Self {
        bitvec::array::BitArray::new(bs.0)
    }
}

macro_rules! impl_bitset_const {
    ($($ty:ty),+) => {$(
        impl<V> BitSet<$ty, V> {
            /// Const constructor from raw bits.
            #[inline]
            pub const fn from_bits(raw: $ty) -> Self {
                Self(raw, PhantomData)
            }

            /// Raw bits accessor (const).
            #[inline]
            pub const fn bits(&self) -> $ty {
                self.0
            }

            #[inline]
            pub const fn as_bits(&self) -> &$ty {
                &self.0
            }

            /// Const constructor from a single bit index.
            #[inline]
            pub const fn from_index(idx: usize) -> Self {
                Self::from_bits((1 as $ty) << idx)
            }

            /// Const constructor from a slice of bit indices.
            #[inline]
            pub const fn from_indices(indices: &[usize]) -> Self {
                let mut raw: $ty = 0;
                let mut i = 0;
                while i < indices.len() {
                    raw |= (1 as $ty) << indices[i];
                    i += 1;
                }
                Self::from_bits(raw)
            }

            #[inline]
            pub const fn len(&self) -> usize {
                self.0.count_ones() as usize
            }

            #[inline]
            pub const fn is_empty(&self) -> bool {
                self.0 == 0
            }

            /// Const bit-index membership test.
            #[inline]
            pub const fn contains(&self, idx: &usize) -> bool {
                self.0 & ((1 as $ty) << *idx) != 0
            }

            #[inline]
            pub const fn is_subset(&self, other: &Self) -> bool {
                self.0 & other.0 == self.0
            }

            #[inline]
            pub const fn is_superset(&self, other: &Self) -> bool {
                self.0 & other.0 == other.0
            }

            #[inline]
            pub const fn is_disjoint(&self, other: &Self) -> bool {
                self.0 & other.0 == 0
            }
        }
    )+};
}
impl_bitset_const!(u8, u16, u32, u64, u128, usize);

impl<A: PrimStore + Copy + core::ops::BitOr<Output = A>, V> core::ops::BitOr for BitSet<A, V> {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0, PhantomData)
    }
}

impl<A: PrimStore + core::ops::BitOrAssign, V> core::ops::BitOrAssign for BitSet<A, V> {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl<A: PrimStore + Copy + core::ops::BitAnd<Output = A>, V> core::ops::BitAnd for BitSet<A, V> {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0, PhantomData)
    }
}

impl<A: PrimStore + core::ops::BitAndAssign, V> core::ops::BitAndAssign for BitSet<A, V> {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl<A: PrimStore + Copy + core::ops::BitXor<Output = A>, V> core::ops::BitXor for BitSet<A, V> {
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0, PhantomData)
    }
}

impl<A: PrimStore + core::ops::BitXorAssign, V> core::ops::BitXorAssign for BitSet<A, V> {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl<A: PrimStore + Copy + core::ops::Not<Output = A>, V> core::ops::Not for BitSet<A, V> {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0, PhantomData)
    }
}

impl<A, V> core::ops::Sub for BitSet<A, V>
where
    A: PrimStore + Copy + core::ops::BitAnd<Output = A> + core::ops::Not<Output = A>,
{
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 & !rhs.0, PhantomData)
    }
}

impl<A, V> core::ops::SubAssign for BitSet<A, V>
where
    A: PrimStore + Copy + core::ops::BitAndAssign + core::ops::Not<Output = A>,
{
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 &= !rhs.0;
    }
}

/// Backward-compatible alias: `ArrayBitSet<A, V, N>` = `BitSet<[A; N], V>`
pub type ArrayBitSet<A, V, const N: usize> = BitSet<[A; N], V>;

// [T; N] arrays — Provide array-specific methods in a separate impl block.

impl<T, V, const N: usize> Deref for BitSet<[T; N], V> {
    type Target = BitSlice<T, V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        BitSlice::from_slice_ref(&self.0)
    }
}

impl<T, V, const N: usize> DerefMut for BitSet<[T; N], V> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        BitSlice::from_slice_mut(&mut self.0)
    }
}

impl<T: PrimStore, V, const N: usize> BitSet<[T; N], V> {
    const ZERO: Self = Self([T::ZERO; N], PhantomData);

    #[inline]
    pub const fn new() -> Self {
        Self::ZERO
    }

    #[inline]
    pub const fn empty() -> Self {
        Self::new()
    }

    #[inline]
    pub fn from_bits(raw: [T; N]) -> Self {
        Self(raw, PhantomData)
    }

    #[inline]
    pub fn from_element(id: V) -> Self
    where
        T: PrimInt,
        V: AsPrimitive<usize>,
    {
        let mut zelf = Self::new();
        zelf.set(id, true);
        zelf
    }

    #[inline]
    pub fn as_bits(&self) -> &[T; N] {
        &self.0
    }

    #[inline]
    pub fn into_bits(self) -> [T; N] {
        self.0
    }

    #[inline]
    pub fn is_subset(&self, other: &Self) -> bool
    where
        T: Copy + core::ops::BitAnd<Output = T> + PartialEq,
    {
        self.0
            .iter()
            .zip(other.0.iter())
            .all(|(a, b)| *a & *b == *a)
    }

    #[inline]
    pub fn is_superset(&self, other: &Self) -> bool
    where
        T: Copy + core::ops::BitAnd<Output = T> + PartialEq,
    {
        other.is_subset(self)
    }

    #[inline]
    pub fn is_disjoint(&self, other: &Self) -> bool
    where
        T: Copy + core::ops::BitAnd<Output = T> + num_traits::Zero,
    {
        self.0
            .iter()
            .zip(other.0.iter())
            .all(|(a, b)| (*a & *b).is_zero())
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self.0.as_ref()
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.0.as_mut()
    }

    #[cfg(feature = "bitvec")]
    #[inline]
    pub fn as_bitvec_array(&self) -> &bitvec::array::BitArray<[T; N]>
    where
        T: bitvec::store::BitStore,
        [T; N]: bitvec::view::BitViewSized,
    {
        // SAFETY: BitArray<[T; N]> is #[repr(transparent)] over [T; N]
        unsafe { &*(&self.0 as *const [T; N] as *const bitvec::array::BitArray<[T; N]>) }
    }

    #[cfg(feature = "bitvec")]
    #[inline]
    pub fn into_bitvec_array(self) -> bitvec::array::BitArray<[T; N]>
    where
        T: bitvec::store::BitStore,
        [T; N]: bitvec::view::BitViewSized,
    {
        bitvec::array::BitArray::new(self.0)
    }
}

#[cfg(feature = "bitvec")]
impl<T, V, const N: usize> From<bitvec::array::BitArray<[T; N]>> for BitSet<[T; N], V>
where
    T: bitvec::store::BitStore,
    [T; N]: bitvec::view::BitViewSized,
{
    #[inline]
    fn from(arr: bitvec::array::BitArray<[T; N]>) -> Self {
        Self(arr.into_inner(), PhantomData)
    }
}

#[cfg(feature = "bitvec")]
impl<T, V, const N: usize> From<BitSet<[T; N], V>> for bitvec::array::BitArray<[T; N]>
where
    T: bitvec::store::BitStore,
    [T; N]: bitvec::view::BitViewSized,
{
    #[inline]
    fn from(bs: BitSet<[T; N], V>) -> Self {
        bitvec::array::BitArray::new(bs.0)
    }
}

impl<T: PartialEq, V, const N: usize> PartialEq for BitSet<[T; N], V> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Eq, V, const N: usize> Eq for BitSet<[T; N], V> {}

impl<T: Ord, V, const N: usize> PartialOrd for BitSet<[T; N], V> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord, V, const N: usize> Ord for BitSet<[T; N], V> {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T: Hash, V, const N: usize> Hash for BitSet<[T; N], V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T: PrimInt, V, const N: usize> core::fmt::Debug for BitSet<[T; N], V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut formatter = f.debug_tuple("BitSet");
        let bits_per = core::mem::size_of::<T>() * 8;
        for (seg_idx, &word) in self.0.iter().enumerate() {
            for bit in 0..bits_per {
                if word & T::one().unsigned_shl(bit as u32) != T::zero() {
                    formatter.field(&(seg_idx * bits_per + bit));
                }
            }
        }
        formatter.finish()
    }
}

impl<T: PrimInt + core::fmt::Binary, V, const N: usize> core::fmt::Binary for BitSet<[T; N], V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let bits_per = core::mem::size_of::<T>() * 8;
        for (i, &word) in self.0.iter().enumerate().rev() {
            if i == N - 1 {
                core::fmt::Binary::fmt(&word, f)?;
            } else {
                write!(f, "{:0>width$b}", word, width = bits_per)?;
            }
        }
        Ok(())
    }
}

impl<T: PrimInt + core::fmt::Octal, V, const N: usize> core::fmt::Octal for BitSet<[T; N], V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Octal doesn't align cleanly to word boundaries, format as combined value
        // by formatting each word with zero-padding
        let width = (core::mem::size_of::<T>() * 8 + 2) / 3; // ceil(bits/3)
        for (i, &word) in self.0.iter().enumerate().rev() {
            if i == N - 1 {
                core::fmt::Octal::fmt(&word, f)?;
            } else {
                write!(f, "{:0>width$o}", word, width = width)?;
            }
        }
        Ok(())
    }
}

impl<T: PrimInt + core::fmt::LowerHex, V, const N: usize> core::fmt::LowerHex
    for BitSet<[T; N], V>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let hex_per = core::mem::size_of::<T>() * 2;
        for (i, &word) in self.0.iter().enumerate().rev() {
            if i == N - 1 {
                core::fmt::LowerHex::fmt(&word, f)?;
            } else {
                write!(f, "{:0>width$x}", word, width = hex_per)?;
            }
        }
        Ok(())
    }
}

impl<T: PrimInt + core::fmt::UpperHex, V, const N: usize> core::fmt::UpperHex
    for BitSet<[T; N], V>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let hex_per = core::mem::size_of::<T>() * 2;
        for (i, &word) in self.0.iter().enumerate().rev() {
            if i == N - 1 {
                core::fmt::UpperHex::fmt(&word, f)?;
            } else {
                write!(f, "{:0>width$X}", word, width = hex_per)?;
            }
        }
        Ok(())
    }
}

impl<T: PrimStore, V, const N: usize> Default for BitSet<[T; N], V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T, V, const N: usize> core::ops::BitOr for BitSet<[T; N], V>
where
    T: PrimStore + core::ops::BitOr<Output = T> + Copy,
{
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self::from_bits(core::array::from_fn(|i| self.0[i] | rhs.0[i]))
    }
}

impl<T, V, const N: usize> core::ops::BitOrAssign for BitSet<[T; N], V>
where
    T: PrimStore + core::ops::BitOrAssign + Copy,
{
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        for i in 0..N {
            self.0[i] |= rhs.0[i];
        }
    }
}

impl<T, V, const N: usize> core::ops::BitAnd for BitSet<[T; N], V>
where
    T: PrimStore + core::ops::BitAnd<Output = T> + Copy,
{
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self::from_bits(core::array::from_fn(|i| self.0[i] & rhs.0[i]))
    }
}

impl<T, V, const N: usize> core::ops::BitAndAssign for BitSet<[T; N], V>
where
    T: PrimStore + core::ops::BitAndAssign + Copy,
{
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        for i in 0..N {
            self.0[i] &= rhs.0[i];
        }
    }
}

impl<T, V, const N: usize> core::ops::BitXor for BitSet<[T; N], V>
where
    T: PrimStore + core::ops::BitXor<Output = T> + Copy,
{
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        Self::from_bits(core::array::from_fn(|i| self.0[i] ^ rhs.0[i]))
    }
}

impl<T, V, const N: usize> core::ops::BitXorAssign for BitSet<[T; N], V>
where
    T: PrimStore + core::ops::BitXorAssign + Copy,
{
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        for i in 0..N {
            self.0[i] ^= rhs.0[i];
        }
    }
}

impl<T, V, const N: usize> core::ops::Not for BitSet<[T; N], V>
where
    T: PrimStore + core::ops::Not<Output = T> + Copy,
{
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self::from_bits(core::array::from_fn(|i| !self.0[i]))
    }
}

impl<T, V, const N: usize> core::ops::Sub for BitSet<[T; N], V>
where
    T: PrimStore + core::ops::BitAnd<Output = T> + core::ops::Not<Output = T> + Copy,
{
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::from_bits(core::array::from_fn(|i| self.0[i] & !rhs.0[i]))
    }
}

impl<T, V, const N: usize> core::ops::SubAssign for BitSet<[T; N], V>
where
    T: PrimStore + Copy + core::ops::Not<Output = T> + core::ops::BitAndAssign,
{
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        for i in 0..N {
            self.0[i] &= !rhs.0[i];
        }
    }
}

impl<'a, T: PrimInt + core::ops::BitAndAssign, V: TryFrom<usize>, const N: usize> IntoIterator
    for &'a BitSet<[T; N], V>
{
    type Item = V;
    type IntoIter = super::slice::BitSliceIter<'a, T, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: PrimInt + core::ops::BitAndAssign, V: TryFrom<usize>, const N: usize> IntoIterator
    for BitSet<[T; N], V>
{
    type Item = V;
    type IntoIter = ArrayBitSetIntoIter<T, V, N>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        super::slice::WordSetIter::new(self.0)
    }
}

/// Owned iterator over set bit positions in a `BitSet<[T; N], V>`.
pub type ArrayBitSetIntoIter<T, V, const N: usize> = super::slice::WordSetIter<[T; N], T, V>;

impl<T: PrimStore, V: AsPrimitive<usize>, const N: usize> core::iter::Extend<V>
    for BitSet<[T; N], V>
{
    fn extend<I: IntoIterator<Item = V>>(&mut self, iter: I) {
        for item in iter {
            self.set(item, true);
        }
    }
}

impl<T: PrimStore, V: AsPrimitive<usize>, const N: usize> FromIterator<V> for BitSet<[T; N], V> {
    fn from_iter<I: IntoIterator<Item = V>>(iter: I) -> Self {
        let mut ret = Self::new();
        for item in iter {
            ret.set(item, true);
        }
        ret
    }
}

impl<T: PrimStore + core::ops::BitOrAssign + Copy, V, const N: usize>
    FromIterator<BitSet<[T; N], V>> for BitSet<[T; N], V>
{
    fn from_iter<I: IntoIterator<Item = Self>>(iter: I) -> Self {
        let mut ret = Self::new();
        for bs in iter {
            for i in 0..N {
                ret.0[i] |= bs.0[i];
            }
        }
        ret
    }
}

impl<T: PrimStore + core::ops::BitOrAssign + Copy, V, const N: usize>
    core::iter::Extend<BitSet<[T; N], V>> for BitSet<[T; N], V>
{
    fn extend<I: IntoIterator<Item = Self>>(&mut self, iter: I) {
        for bs in iter {
            for i in 0..N {
                self.0[i] |= bs.0[i];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use proptest::prelude::*;
    use rand::Rng;

    #[test]
    fn test_size_prim() {
        assert_eq!(
            core::mem::size_of::<BitSet<u64, i32>>(),
            core::mem::size_of::<u64>()
        );
        assert_eq!(
            core::mem::size_of::<BitSet<u32, i32>>(),
            core::mem::size_of::<u32>()
        );
        assert_eq!(
            core::mem::size_of::<BitSet<u8, i32>>(),
            core::mem::size_of::<u8>()
        );
    }

    const SET: BitSet<u64, usize> = BitSet::<u64, usize>::from_indices(&[3, 7, 42]);
    const RAW: BitSet<u64, usize> = BitSet::<u64, usize>::from_bits(0b1010);
    const SINGLE: BitSet<u64, usize> = BitSet::<u64, usize>::from_index(5);
    const EMPTY: BitSet<u64, usize> = BitSet::<u64, usize>::new();

    // from_bits / bits
    const _: () = assert!(RAW.bits() == 0b1010);

    // from_indices / from_index
    const _: () = assert!(SET.contains(&3));
    const _: () = assert!(SET.contains(&7));
    const _: () = assert!(SET.contains(&42));
    const _: () = assert!(!SET.contains(&0));
    const _: () = assert!(SINGLE.contains(&5));
    const _: () = assert!(!SINGLE.contains(&4));

    // len / is_empty
    const _: () = assert!(SET.len() == 3);
    const _: () = assert!(RAW.len() == 2);
    const _: () = assert!(SINGLE.len() == 1);
    const _: () = assert!(EMPTY.is_empty());
    const _: () = assert!(!SET.is_empty());
    const _: () = assert!(EMPTY.is_empty());

    // is_subset / is_superset / is_disjoint
    const SUPERSET: BitSet<u64, usize> = BitSet::<u64, usize>::from_indices(&[3, 7, 42, 50]);
    const DISJOINT: BitSet<u64, usize> = BitSet::<u64, usize>::from_indices(&[0, 1]);
    const _: () = assert!(SET.is_subset(&SUPERSET));
    const _: () = assert!(!SUPERSET.is_subset(&SET));
    const _: () = assert!(SUPERSET.is_superset(&SET));
    const _: () = assert!(!SET.is_superset(&SUPERSET));
    const _: () = assert!(SET.is_disjoint(&DISJOINT));
    const _: () = assert!(!SET.is_disjoint(&SUPERSET));
    const _: () = assert!(EMPTY.is_disjoint(&SET));

    // ── Runtime verification (same methods, non‐const path) ─

    #[test]
    fn test_prim_const_methods_at_runtime() {
        // from_bits / bits roundtrip
        let bs = BitSet::<u64, usize>::from_bits(0xDEAD);
        assert_eq!(bs.bits(), 0xDEAD);

        // from_index / from_indices
        let single = BitSet::<u32, usize>::from_index(10);
        assert!(single.contains(&10));
        assert_eq!(single.len(), 1);

        let multi = BitSet::<u32, usize>::from_indices(&[1, 5, 9]);
        assert!(multi.contains(&1));
        assert!(multi.contains(&5));
        assert!(multi.contains(&9));
        assert!(!multi.contains(&2));
        assert_eq!(multi.len(), 3);

        // is_empty
        assert!(BitSet::<u64, usize>::new().is_empty());
        assert!(!multi.is_empty());

        // is_subset / is_superset / is_disjoint
        let superset = BitSet::<u32, usize>::from_indices(&[1, 5, 9, 20]);
        let disjoint = BitSet::<u32, usize>::from_indices(&[2, 3]);
        assert!(multi.is_subset(&superset));
        assert!(!superset.is_subset(&multi));
        assert!(superset.is_superset(&multi));
        assert!(multi.is_disjoint(&disjoint));
        assert!(!multi.is_disjoint(&superset));
    }

    #[test]
    fn test_prim_deref_methods() {
        let mut bs = BitSet::<u64, usize>::new();
        bs.insert(3);
        bs.insert(7);
        bs.insert(42);

        // BitSlice::contains (takes &V, generic)
        assert!(bs.contains(&3));
        assert!(bs.contains(&7));
        assert!(!bs.contains(&0));

        // BitSlice::remove
        assert!(bs.remove(7));
        assert!(!bs.contains(&7));

        // BitSlice::iter
        let items: Vec<usize> = bs.iter().collect();
        assert!(items.contains(&3));
        assert!(items.contains(&42));

        // BitSlice::clear
        bs.clear();
        assert!(bs.is_empty());
    }

    #[test]
    fn test_bitflagset() {
        use crate::BitFlagSet;

        crate::bitflag! {
            #[derive(Clone, Copy, PartialEq, Eq, Debug)]
            #[repr(u8)]
            enum Color {
                Red = 0,
                Green = 1,
                Blue = 2,
            }
        }

        crate::bitflagset!(#[derive(Clone, Copy, PartialEq, Eq)] struct ColorSet(u8) : Color);

        // inherent method calls
        let set = ColorSet::from_element(Color::Green);
        assert_eq!(set.first(), Some(Color::Green));
        assert_eq!(set.last(), Some(Color::Green));
        assert_eq!(set.len(), 1);
        assert!(!set.is_empty());
        assert!(set.contains(&Color::Green));
        assert!(!set.contains(&Color::Red));
        let v: Vec<Color> = set.iter().collect();
        assert_eq!(v, vec![Color::Green]);
        assert_eq!(set.to_vec(), vec![Color::Green]);

        // trait method calls
        let set2 = <ColorSet as BitFlagSet<Color, u8>>::from_element(Color::Blue);
        assert_eq!(BitFlagSet::first(&set2), Some(Color::Blue));
        assert_eq!(BitFlagSet::last(&set2), Some(Color::Blue));
        assert_eq!(BitFlagSet::len(&set2), 1);
        assert!(!BitFlagSet::is_empty(&set2));
        assert!(BitFlagSet::contains(&set2, &Color::Blue));
        let v2: Vec<Color> = BitFlagSet::iter(&set2).collect();
        assert_eq!(v2, vec![Color::Blue]);
        assert_eq!(BitFlagSet::to_vec(&set2), vec![Color::Blue]);

        // insert / remove
        let mut set3 = ColorSet::empty();
        assert!(set3.insert(Color::Red));
        assert!(!set3.insert(Color::Red)); // already present
        assert!(set3.contains(&Color::Red));
        assert!(set3.remove(Color::Red));
        assert!(!set3.remove(Color::Red)); // already absent
        assert!(!set3.contains(&Color::Red));

        // is_subset / is_superset / is_disjoint
        let rgb = ColorSet::from_slice(&[Color::Red, Color::Green, Color::Blue]);
        let rg = ColorSet::from_slice(&[Color::Red, Color::Green]);
        let b = ColorSet::from_element(Color::Blue);
        assert!(rg.is_subset(&rgb));
        assert!(!rgb.is_subset(&rg));
        assert!(rgb.is_superset(&rg));
        assert!(!rg.is_superset(&rgb));
        assert!(rg.is_disjoint(&b));
        assert!(!rg.is_disjoint(&rgb));

        // BitXor (symmetric difference)
        let xor = rgb ^ rg;
        assert_eq!(xor, b);
        let mut xor_assign = rgb;
        xor_assign ^= rg;
        assert_eq!(xor_assign, b);

        // Not (complement)
        let not_empty = !ColorSet::empty();
        assert!(not_empty.contains(&Color::Red));
        assert!(not_empty.contains(&Color::Green));
        assert!(not_empty.contains(&Color::Blue));

        // Extend
        let mut set4 = ColorSet::empty();
        set4.extend([Color::Red, Color::Blue]);
        assert_eq!(set4.len(), 2);
        assert!(set4.contains(&Color::Red));
        assert!(set4.contains(&Color::Blue));

        // compound operations
        let combined = set | set2;
        assert_eq!(combined.len(), 2);
        assert_eq!(combined.first(), Some(Color::Green));
        assert_eq!(combined.last(), Some(Color::Blue));

        // From conversions
        let bs: BitSet<u8, Color> = rgb.into();
        assert_eq!(bs.len(), 3);
        let back: ColorSet = bs.into();
        assert_eq!(back, rgb);

        // Additional inherent API coverage
        assert!(ColorSet::from_bits(0b111).is_some());
        assert_eq!(ColorSet::from_bits_truncate(0xFF), ColorSet::all());
        let unchecked = unsafe { ColorSet::from_bits_unchecked(0b001) };
        assert!(unchecked.contains(&Color::Red));
        let mut all = ColorSet::all();
        assert!(all.is_all());
        all.set(Color::Blue, false);
        all.toggle(Color::Blue);
        all.clear();
        assert!(all.is_empty());
        let _ = ColorSet::from_slice(&[Color::Red]).iter_names().count();
    }

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
        fn flag_set_iter_32(indexes in arb_indexes(32)) {
            let mut flags = BitSet::<u32, usize>::new();
            for idx in indexes.iter() {
                flags.set(*idx, true);
            }
            assert_set_result(&indexes, flags.drain());
        }
        #[test]
        fn flag_set_iter_64(indexes in arb_indexes(64)) {
            let mut flags = BitSet::<u64, usize>::new();
            for idx in indexes.iter() {
                flags.set(*idx, true);
            }
            assert_set_result(&indexes, flags.drain());
        }

        #[test]
        fn parity_bitarray_128(ops in prop::collection::vec((any::<bool>(), 0..128usize), 0..200)) {
            use bitvec::array::BitArray;
            use bitvec::order::Lsb0;

            let mut ours = BitSet::<[u64; 2], usize>::new();
            let mut bv = BitArray::<[u64; 2], Lsb0>::ZERO;

            for &(insert, idx) in &ops {
                if insert {
                    ours.insert(idx);
                    bv.set(idx, true);
                } else {
                    ours.remove(idx);
                    bv.set(idx, false);
                }
            }

            prop_assert_eq!(ours.len(), bv.count_ones(), "len mismatch");
            prop_assert_eq!(ours.is_empty(), bv.not_any(), "is_empty mismatch");

            for i in 0..128 {
                prop_assert_eq!(ours.contains(&i), bv[i], "contains mismatch at {}", i);
            }

            let ours_bits: Vec<usize> = ours.iter().collect();
            let bv_bits: Vec<usize> = bv.iter_ones().collect();
            prop_assert_eq!(ours_bits, bv_bits, "iter mismatch");
        }

        #[test]
        fn parity_bitarray_1024(ops in prop::collection::vec((any::<bool>(), 0..1024usize), 0..200)) {
            use bitvec::array::BitArray;
            use bitvec::order::Lsb0;

            let mut ours = BitSet::<[u64; 16], usize>::new();
            let mut bv = BitArray::<[u64; 16], Lsb0>::ZERO;

            for &(insert, idx) in &ops {
                if insert {
                    ours.insert(idx);
                    bv.set(idx, true);
                } else {
                    ours.remove(idx);
                    bv.set(idx, false);
                }
            }

            prop_assert_eq!(ours.len(), bv.count_ones(), "len mismatch");
            prop_assert_eq!(ours.is_empty(), bv.not_any(), "is_empty mismatch");

            for i in 0..1024 {
                prop_assert_eq!(ours.contains(&i), bv[i], "contains mismatch at {}", i);
            }

            let ours_bits: Vec<usize> = ours.iter().collect();
            let bv_bits: Vec<usize> = bv.iter_ones().collect();
            prop_assert_eq!(ours_bits, bv_bits, "iter mismatch");
        }
    }

    #[test]
    fn test_size_array() {
        assert_eq!(
            core::mem::size_of::<BitSet<[u64; 11], i32>>(),
            core::mem::size_of::<[u64; 11]>()
        );
    }

    #[test]
    fn test_basic() {
        let mut bs = BitSet::<[u64; 2], usize>::new();
        assert!(bs.is_empty());
        bs.set(0, true);
        bs.set(63, true);
        bs.set(64, true);
        bs.set(127, true);
        assert!(!bs.is_empty());

        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 63, 64, 127]);
    }

    #[test]
    fn test_array_boundary_values() {
        // [u64; 4] = 256 bits, max index = 255
        let mut bs = BitSet::<[u64; 4], usize>::new();
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

        // [u64; 16] = 1024 bits, max index = 1023
        let mut bs = BitSet::<[u64; 16], usize>::new();
        bs.insert(0);
        bs.insert(1023);
        bs.insert(512);
        assert_eq!(bs.len(), 3);

        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 512, 1023]);

        assert!(bs.remove(1023));
        assert!(!bs.contains(&1023));

        // fill every word boundary
        let mut bs = BitSet::<[u64; 4], usize>::new();
        for i in (0..256).step_by(64) {
            bs.insert(i);
        }
        // also insert last bit of each word
        for i in (63..256).step_by(64) {
            bs.insert(i);
        }
        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![0, 63, 64, 127, 128, 191, 192, 255]);
    }

    #[test]
    fn test_alias() {
        let mut bs = ArrayBitSet::<u64, usize, 2>::new();
        bs.set(5, true);
        bs.set(70, true);
        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![5, 70]);
    }

    #[test]
    fn test_bitor() {
        let a = BitSet::<[u64; 2], usize>::from_element(10);
        let b = BitSet::<[u64; 2], usize>::from_element(100);
        let c = a | b;
        let items: Vec<usize> = c.iter().collect();
        assert_eq!(items, vec![10, 100]);
    }

    #[test]
    fn test_array_set_relations() {
        let mut a = BitSet::<[u64; 2], usize>::new();
        a.set(1, true);
        a.set(65, true);

        let mut b = BitSet::<[u64; 2], usize>::new();
        b.set(1, true);
        b.set(65, true);
        b.set(100, true);

        let mut c = BitSet::<[u64; 2], usize>::new();
        c.set(2, true);
        c.set(66, true);

        assert!(a.is_subset(&b));
        assert!(!b.is_subset(&a));
        assert!(b.is_superset(&a));
        assert!(!a.is_superset(&b));
        assert!(a.is_disjoint(&c));
        assert!(!a.is_disjoint(&b));
    }

    #[test]
    fn test_prim_toggle() {
        let mut bs = BitSet::<u64, usize>::new();
        bs.insert(3);
        bs.toggle(3);
        assert!(!bs.contains(&3));
        bs.toggle(3);
        assert!(bs.contains(&3));
        bs.toggle(10);
        assert!(bs.contains(&10));
    }

    #[test]
    fn test_array_toggle() {
        let mut bs = BitSet::<[u64; 2], usize>::new();
        bs.insert(5);
        bs.insert(70);
        bs.toggle(5);
        assert!(!bs.contains(&5));
        bs.toggle(70);
        assert!(!bs.contains(&70));
        bs.toggle(100);
        assert!(bs.contains(&100));
    }

    #[test]
    fn test_prim_format_traits() {
        use alloc::format;
        let bs = BitSet::<u64, usize>::from_bits(0b1010);
        assert_eq!(format!("{bs:b}"), "1010");
        assert_eq!(format!("{bs:o}"), "12");
        assert_eq!(format!("{bs:x}"), "a");
        assert_eq!(format!("{bs:X}"), "A");
    }

    #[test]
    fn test_prim_from_iter_self() {
        let sets = [
            BitSet::<u64, usize>::from_index(3),
            BitSet::<u64, usize>::from_index(7),
            BitSet::<u64, usize>::from_index(42),
        ];
        let merged: BitSet<u64, usize> = sets.into_iter().collect();
        assert!(merged.contains(&3));
        assert!(merged.contains(&7));
        assert!(merged.contains(&42));
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_prim_extend_self() {
        let mut bs = BitSet::<u64, usize>::from_index(1);
        bs.extend([
            BitSet::<u64, usize>::from_index(5),
            BitSet::<u64, usize>::from_index(10),
        ]);
        assert!(bs.contains(&1));
        assert!(bs.contains(&5));
        assert!(bs.contains(&10));
    }

    #[test]
    fn test_array_from_iter_self() {
        let sets = [
            BitSet::<[u64; 2], usize>::from_element(10),
            BitSet::<[u64; 2], usize>::from_element(100),
        ];
        let merged: BitSet<[u64; 2], usize> = sets.into_iter().collect();
        assert!(merged.contains(&10));
        assert!(merged.contains(&100));
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_array_extend_self() {
        let mut bs = BitSet::<[u64; 2], usize>::from_element(5);
        bs.extend([BitSet::<[u64; 2], usize>::from_element(70)]);
        assert!(bs.contains(&5));
        assert!(bs.contains(&70));
    }

    #[test]
    fn test_iter_size_hint_and_count_remaining() {
        let mut bs = BitSet::<[u64; 2], usize>::new();
        bs.insert(1);
        bs.insert(3);
        bs.insert(64);
        bs.insert(100);

        let mut iter = bs.iter();
        assert_eq!(iter.size_hint(), (4, Some(4)));
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.size_hint(), (3, Some(3)));
        assert_eq!(iter.next(), Some(3));
        assert_eq!(iter.size_hint(), (2, Some(2)));
        assert_eq!(iter.count(), 2);
    }

    #[test]
    fn test_retain() {
        let mut bs = BitSet::<[u64; 2], usize>::new();
        for i in [1, 3, 5, 64, 66, 100] {
            bs.insert(i);
        }
        // keep only even positions
        bs.retain(|v| v % 2 == 0);
        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![64, 66, 100]);
    }

    #[test]
    fn test_prim_retain() {
        let mut bs = BitSet::<u64, usize>::new();
        for i in [0, 1, 2, 3, 4, 5] {
            bs.insert(i);
        }
        bs.retain(|v| v >= 3);
        let items: Vec<usize> = bs.iter().collect();
        assert_eq!(items, vec![3, 4, 5]);
    }

    #[test]
    fn test_drain_slice() {
        let mut bs = BitSet::<[u64; 2], usize>::new();
        bs.insert(1);
        bs.insert(65);
        bs.insert(100);

        let items: Vec<usize> = bs.drain().collect();
        assert_eq!(items, vec![1, 65, 100]);
        assert!(bs.is_empty());
    }

    #[test]
    fn test_drain_drop_clears() {
        let mut bs = BitSet::<[u64; 2], usize>::new();
        bs.insert(1);
        bs.insert(65);
        bs.insert(100);

        // Take only the first element, then drop
        let mut drain = bs.drain();
        assert_eq!(drain.next(), Some(1));
        drop(drain);
        assert!(bs.is_empty());
    }

    #[test]
    fn test_append() {
        let mut a = BitSet::<[u64; 2], usize>::new();
        a.insert(1);
        a.insert(65);

        let mut b = BitSet::<[u64; 2], usize>::new();
        b.insert(2);
        b.insert(65);
        b.insert(100);

        a.append(&mut b);
        assert_eq!(a.len(), 4); // {1, 2, 65, 100}
        assert!(a.contains(&1));
        assert!(a.contains(&2));
        assert!(a.contains(&65));
        assert!(a.contains(&100));
        assert!(b.is_empty());
    }

    #[test]
    fn test_difference() {
        let mut a = BitSet::<[u64; 2], usize>::new();
        a.insert(1);
        a.insert(5);
        a.insert(65);

        let mut b = BitSet::<[u64; 2], usize>::new();
        b.insert(5);
        b.insert(10);
        b.insert(65);

        let diff: Vec<usize> = a.difference(&b).collect();
        assert_eq!(diff, vec![1]);
    }

    #[test]
    fn test_intersection() {
        let mut a = BitSet::<[u64; 2], usize>::new();
        a.insert(1);
        a.insert(5);
        a.insert(65);

        let mut b = BitSet::<[u64; 2], usize>::new();
        b.insert(5);
        b.insert(10);
        b.insert(65);

        let inter: Vec<usize> = a.intersection(&b).collect();
        assert_eq!(inter, vec![5, 65]);
    }

    #[test]
    fn test_union_iter() {
        let mut a = BitSet::<[u64; 2], usize>::new();
        a.insert(1);
        a.insert(5);

        let mut b = BitSet::<[u64; 2], usize>::new();
        b.insert(5);
        b.insert(70);

        let uni: Vec<usize> = a.union(&b).collect();
        assert_eq!(uni, vec![1, 5, 70]);
    }

    #[test]
    fn test_symmetric_difference() {
        let mut a = BitSet::<[u64; 2], usize>::new();
        a.insert(1);
        a.insert(5);
        a.insert(65);

        let mut b = BitSet::<[u64; 2], usize>::new();
        b.insert(5);
        b.insert(10);
        b.insert(65);

        let sym_diff: Vec<usize> = a.symmetric_difference(&b).collect();
        assert_eq!(sym_diff, vec![1, 10]);
    }

    #[test]
    fn test_array_into_iter_owned() {
        let mut bs = BitSet::<[u64; 2], usize>::new();
        bs.insert(5);
        bs.insert(70);
        bs.insert(100);
        let items: Vec<usize> = bs.into_iter().collect();
        assert_eq!(items, vec![5, 70, 100]);
    }

    #[test]
    fn test_array_into_iter_for_loop() {
        let mut bs = BitSet::<[u64; 2], usize>::new();
        bs.insert(3);
        bs.insert(64);
        let mut result = Vec::new();
        for item in bs {
            result.push(item);
        }
        assert_eq!(result, vec![3, 64]);
    }
}
