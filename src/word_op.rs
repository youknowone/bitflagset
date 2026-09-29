use core::iter::{ExactSizeIterator, FusedIterator};
use core::marker::PhantomData;
use core::ops::BitAndAssign;
use num_traits::PrimInt;

use super::bitset::PrimBitSetIter;

/// `word * bits_per_word + bit_in_word` as `V`.
///
/// `idx` is in range by construction. Panics if `V::try_from` rejects it.
#[inline(always)]
pub(crate) fn bit_index<V: TryFrom<usize>>(idx: usize) -> V {
    if let Ok(value) = V::try_from(idx) {
        value
    } else {
        // Iterators only pass an in-range index. Panic when `TryFrom<usize>`
        // for `V` rejects that index.
        unreachable!("bit index does not fit in the iterator item");
    }
}

/// Lower bound is the already-loaded word. Each word not yet loaded contributes
/// at most `bits_per` bits, so a later store cannot pass the upper bound.
/// `None` means that product does not fit in `usize`.
#[inline]
pub(crate) fn conservative_size_hint(
    current: usize,
    unseen_words: usize,
    bits_per: usize,
) -> (usize, Option<usize>) {
    let upper = unseen_words
        .checked_mul(bits_per)
        .and_then(|bits| current.checked_add(bits));
    (current, upper)
}

/// `size_hint` for [`WordOpIter`]. Monomorphized; `next` does not branch on it.
pub(crate) trait LenHint: Sized {
    fn apply(
        current: usize,
        unseen_words: usize,
        bits_per: usize,
        exact: impl FnOnce() -> usize,
    ) -> (usize, Option<usize>);
}

/// Immutable words: `size_hint` is exact.
pub(crate) struct Exact;

impl LenHint for Exact {
    #[inline(always)]
    fn apply(
        _current: usize,
        _unseen_words: usize,
        _bits_per: usize,
        exact: impl FnOnce() -> usize,
    ) -> (usize, Option<usize>) {
        let len = exact();
        (len, Some(len))
    }
}

/// Atomic loads: do not read words `next` has not copied yet.
pub(crate) struct Bounded;

impl LenHint for Bounded {
    #[inline(always)]
    fn apply(
        current: usize,
        unseen_words: usize,
        bits_per: usize,
        _exact: impl FnOnce() -> usize,
    ) -> (usize, Option<usize>) {
        conservative_size_hint(current, unseen_words, bits_per)
    }
}

/// Visit set bits low-to-high. `bits` is the still-set mask, not a copy of the
/// original word, so a partially consumed word keeps the right positions.
#[inline]
pub(crate) fn fold_word_bits<Word, V, Acc, F>(
    mut acc: Acc,
    mut bits: Word,
    base: usize,
    f: &mut F,
) -> Acc
where
    Word: PrimInt + BitAndAssign,
    V: TryFrom<usize>,
    F: FnMut(Acc, V) -> Acc,
{
    while !bits.is_zero() {
        let pos = bits.trailing_zeros() as usize;
        let mask = Word::one().unsigned_shl(pos as u32);
        bits &= !mask;
        acc = f(acc, bit_index(base + pos));
    }
    acc
}

/// Set-op iterator over two word slices.
///
/// The shared prefix is zipped directly. At most one side extends past that
/// prefix within `len`; those tail words are combined with a zero word.
pub(crate) struct WordOpIter<'a, W, Word, V, F, L, H>
where
    Word: PrimInt,
{
    a_prefix: &'a [W],
    b_prefix: &'a [W],
    tail: &'a [W],
    tail_from_a: bool,
    op: F,
    load: L,
    current: PrimBitSetIter<Word, usize>,
    current_base: usize,
    next_base: usize,
    _marker: PhantomData<V>,
    _hint: PhantomData<H>,
}

impl<'a, W, Word, V, F, L, H> WordOpIter<'a, W, Word, V, F, L, H>
where
    Word: PrimInt + BitAndAssign,
    F: Fn(Word, Word) -> Word,
    L: Fn(&W) -> Word,
{
    #[inline]
    fn new(a: &'a [W], b: &'a [W], len: usize, op: F, load: L) -> Self {
        let prefix_len = a.len().min(b.len()).min(len);
        let a_end = a.len().min(len);
        let b_end = b.len().min(len);
        // `len` never needs both tails: prefix already covers the shorter side.
        let (tail, tail_from_a) = if a_end > prefix_len {
            (&a[prefix_len..a_end], true)
        } else {
            (&b[prefix_len..b_end], false)
        };
        Self {
            a_prefix: &a[..prefix_len],
            b_prefix: &b[..prefix_len],
            tail,
            tail_from_a,
            op,
            load,
            current: PrimBitSetIter::empty(),
            current_base: 0,
            next_base: 0,
            _marker: PhantomData,
            _hint: PhantomData,
        }
    }

    /// Words `next` has not loaded: the rest of the paired prefix, plus the tail.
    #[inline]
    fn unseen_words(&self) -> usize {
        self.a_prefix.len() + self.tail.len()
    }

    #[inline]
    fn bits_per() -> usize {
        core::mem::size_of::<Word>() * 8
    }

    #[inline]
    fn take_prefix(&mut self) -> Option<Word> {
        let (wa, a_rest) = self.a_prefix.split_first()?;
        let (wb, b_rest) = self.b_prefix.split_first()?;
        let combined = (self.op)((self.load)(wa), (self.load)(wb));
        self.a_prefix = a_rest;
        self.b_prefix = b_rest;
        Some(combined)
    }

    #[inline]
    fn take_tail(&mut self) -> Option<Word> {
        let (w, rest) = self.tail.split_first()?;
        let word = (self.load)(w);
        let zero = Word::zero();
        let combined = if self.tail_from_a {
            (self.op)(word, zero)
        } else {
            (self.op)(zero, word)
        };
        self.tail = rest;
        Some(combined)
    }

    #[inline]
    fn set_current(&mut self, combined: Word) {
        self.current = PrimBitSetIter::from_raw(combined);
        self.current_base = self.next_base;
        self.next_base += Self::bits_per();
    }

    #[inline]
    fn prefix_ones(&self) -> usize {
        self.a_prefix
            .iter()
            .zip(self.b_prefix.iter())
            .map(|(wa, wb)| (self.op)((self.load)(wa), (self.load)(wb)).count_ones() as usize)
            .sum()
    }

    #[inline]
    fn tail_ones(&self) -> usize {
        let zero = Word::zero();
        if self.tail_from_a {
            self.tail
                .iter()
                .map(|w| (self.op)((self.load)(w), zero).count_ones() as usize)
                .sum()
        } else {
            self.tail
                .iter()
                .map(|w| (self.op)(zero, (self.load)(w)).count_ones() as usize)
                .sum()
        }
    }

    #[inline]
    fn remaining_len(&self) -> usize {
        self.current.len() + self.prefix_ones() + self.tail_ones()
    }
}

impl<'a, W, Word, V, F, L> WordOpIter<'a, W, Word, V, F, L, Exact>
where
    Word: PrimInt + BitAndAssign,
    F: Fn(Word, Word) -> Word,
    L: Fn(&W) -> Word,
{
    #[inline]
    pub(crate) fn exact(a: &'a [W], b: &'a [W], len: usize, op: F, load: L) -> Self {
        Self::new(a, b, len, op, load)
    }
}

impl<'a, W, Word, V, F, L> WordOpIter<'a, W, Word, V, F, L, Bounded>
where
    Word: PrimInt + BitAndAssign,
    F: Fn(Word, Word) -> Word,
    L: Fn(&W) -> Word,
{
    #[inline]
    pub(crate) fn bounded(a: &'a [W], b: &'a [W], len: usize, op: F, load: L) -> Self {
        Self::new(a, b, len, op, load)
    }
}

impl<W, Word, V, F, L, H> Iterator for WordOpIter<'_, W, Word, V, F, L, H>
where
    Word: PrimInt + BitAndAssign,
    V: TryFrom<usize>,
    F: Fn(Word, Word) -> Word,
    L: Fn(&W) -> Word,
    H: LenHint,
{
    type Item = V;

    #[inline]
    fn next(&mut self) -> Option<V> {
        loop {
            if let Some(pos) = self.current.next() {
                return Some(bit_index(self.current_base + pos));
            }
            if let Some(combined) = self.take_prefix() {
                self.set_current(combined);
                continue;
            }
            if let Some(combined) = self.take_tail() {
                self.set_current(combined);
                continue;
            }
            return None;
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        H::apply(
            self.current.len(),
            self.unseen_words(),
            Self::bits_per(),
            || self.remaining_len(),
        )
    }

    #[inline]
    fn count(self) -> usize {
        self.remaining_len()
    }

    #[inline]
    fn fold<Acc, G>(mut self, mut acc: Acc, mut g: G) -> Acc
    where
        G: FnMut(Acc, V) -> Acc,
    {
        let bits_per = Self::bits_per();
        acc = fold_word_bits(acc, self.current.0, self.current_base, &mut g);
        while let Some(combined) = self.take_prefix() {
            let base = self.next_base;
            self.next_base += bits_per;
            acc = fold_word_bits(acc, combined, base, &mut g);
        }
        while let Some(combined) = self.take_tail() {
            let base = self.next_base;
            self.next_base += bits_per;
            acc = fold_word_bits(acc, combined, base, &mut g);
        }
        acc
    }
}

impl<W, Word, V, F, L> ExactSizeIterator for WordOpIter<'_, W, Word, V, F, L, Exact>
where
    Word: PrimInt + BitAndAssign,
    V: TryFrom<usize>,
    F: Fn(Word, Word) -> Word,
    L: Fn(&W) -> Word,
{
    #[inline]
    fn len(&self) -> usize {
        self.remaining_len()
    }
}

impl<W, Word, V, F, L, H> FusedIterator for WordOpIter<'_, W, Word, V, F, L, H>
where
    Word: PrimInt + BitAndAssign,
    V: TryFrom<usize>,
    F: Fn(Word, Word) -> Word,
    L: Fn(&W) -> Word,
    H: LenHint,
{
}

#[cfg(test)]
mod tests {
    use core::cell::Cell;
    use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    use alloc::vec;
    use alloc::vec::Vec;

    use crate::{AtomicBitSet, AtomicBitSlice, BitSet, BitSlice};

    fn build<const N: usize>(bits: &[usize]) -> BitSet<[u64; N], usize> {
        let mut set = BitSet::<[u64; N], usize>::new();
        for bit in bits {
            set.insert(*bit);
        }
        set
    }

    fn build_atomic<const N: usize>(bits: &[usize]) -> AtomicBitSet<[AtomicU64; N], usize> {
        let set = AtomicBitSet::<[AtomicU64; N], usize>::new();
        for bit in bits {
            set.insert(*bit);
        }
        set
    }

    fn push_all(iter: impl Iterator<Item = usize>) -> Vec<usize> {
        iter.fold(Vec::new(), |mut acc, item| {
            acc.push(item);
            acc
        })
    }

    fn assert_exact_hint(hint: (usize, Option<usize>), actual: usize) {
        assert_eq!(hint, (actual, Some(actual)));
    }

    fn assert_bounded_hint(hint: (usize, Option<usize>), actual: usize) {
        let (lower, upper) = hint;
        let Some(upper) = upper else {
            panic!("size_hint {hint:?} has no upper bound");
        };
        assert!(
            lower <= actual && actual <= upper,
            "size_hint ({lower}, Some({upper})) does not cover {actual}"
        );
    }

    fn assert_reducers<I>(
        mut make: impl FnMut() -> I,
        mut check: impl FnMut((usize, Option<usize>), usize),
    ) where
        I: Iterator<Item = usize>,
    {
        let expected: Vec<usize> = make().collect();

        let iter = make();
        check(iter.size_hint(), expected.len());
        assert_eq!(iter.count(), expected.len());
        assert_eq!(push_all(make()), expected);

        if !expected.is_empty() {
            let mut iter = make();
            assert_eq!(iter.next(), Some(expected[0]));
            let rest = &expected[1..];
            check(iter.size_hint(), rest.len());
            assert_eq!(push_all(iter), rest);

            let mut iter = make();
            assert_eq!(iter.next(), Some(expected[0]));
            assert_eq!(iter.count(), rest.len());
        }

        let split = expected.iter().take_while(|bit| **bit < 64).count();
        let mut iter = make();
        for _ in 0..split {
            assert!(iter.next().is_some());
        }
        let rest = &expected[split..];
        check(iter.size_hint(), rest.len());
        assert_eq!(iter.count(), rest.len());

        let mut iter = make();
        for _ in 0..split {
            assert!(iter.next().is_some());
        }
        assert_eq!(push_all(iter), rest);

        let mut iter = make();
        while iter.next().is_some() {}
        assert_eq!(iter.next(), None);
        check(iter.size_hint(), 0);
        assert_eq!(iter.count(), 0);
    }

    fn assert_exact_reducers<I>(make: impl FnMut() -> I)
    where
        I: Iterator<Item = usize>,
    {
        assert_reducers(make, assert_exact_hint);
    }

    fn assert_bounded_reducers<I>(make: impl FnMut() -> I)
    where
        I: Iterator<Item = usize>,
    {
        assert_reducers(make, assert_bounded_hint);
    }

    fn word_at(words: &[u64], index: usize) -> u64 {
        words.get(index).copied().unwrap_or(0)
    }

    fn manual_union(a: &[u64], b: &[u64]) -> usize {
        (0..a.len().max(b.len()))
            .map(|i| (word_at(a, i) | word_at(b, i)).count_ones() as usize)
            .sum()
    }

    fn manual_difference(a: &[u64], b: &[u64]) -> usize {
        a.iter()
            .enumerate()
            .map(|(i, wa)| (*wa & !word_at(b, i)).count_ones() as usize)
            .sum()
    }

    fn manual_sym(a: &[u64], b: &[u64]) -> usize {
        (0..a.len().max(b.len()))
            .map(|i| (word_at(a, i) ^ word_at(b, i)).count_ones() as usize)
            .sum()
    }

    fn manual_intersection(a: &[u64], b: &[u64]) -> usize {
        (0..a.len().min(b.len()))
            .map(|i| (a[i] & b[i]).count_ones() as usize)
            .sum()
    }

    fn check_slice(a: &BitSlice<u64, usize>, b: &BitSlice<u64, usize>) {
        let aw = a.raw_words();
        let bw = b.raw_words();
        assert_exact_reducers(|| a.union(b));
        assert_exact_reducers(|| b.union(a));
        assert_exact_reducers(|| a.difference(b));
        assert_exact_reducers(|| b.difference(a));
        assert_exact_reducers(|| a.symmetric_difference(b));
        assert_exact_reducers(|| b.symmetric_difference(a));
        assert_exact_reducers(|| a.intersection(b));
        assert_exact_reducers(|| b.intersection(a));
        assert_eq!(a.union(b).count(), manual_union(aw, bw));
        assert_eq!(b.union(a).count(), manual_union(bw, aw));
        assert_eq!(a.difference(b).count(), manual_difference(aw, bw));
        assert_eq!(b.difference(a).count(), manual_difference(bw, aw));
        assert_eq!(a.symmetric_difference(b).count(), manual_sym(aw, bw));
        assert_eq!(b.symmetric_difference(a).count(), manual_sym(bw, aw));
        assert_eq!(a.intersection(b).count(), manual_intersection(aw, bw));
        assert_eq!(b.intersection(a).count(), manual_intersection(bw, aw));
    }

    fn atomic_words(set: &AtomicBitSlice<AtomicU64, usize>) -> Vec<u64> {
        set.as_raw_slice()
            .iter()
            .map(|word| word.load(Ordering::Relaxed))
            .collect()
    }

    fn fresh_cap(words: usize) -> usize {
        words * 64
    }

    fn check_atomic(a: &AtomicBitSlice<AtomicU64, usize>, b: &AtomicBitSlice<AtomicU64, usize>) {
        let aw = atomic_words(a);
        let bw = atomic_words(b);
        assert_eq!(a.iter().size_hint(), (0, Some(fresh_cap(aw.len()))));
        assert_eq!(
            a.union(b).size_hint(),
            (0, Some(fresh_cap(aw.len().max(bw.len()))))
        );
        assert_eq!(
            b.union(a).size_hint(),
            (0, Some(fresh_cap(aw.len().max(bw.len()))))
        );
        assert_eq!(a.difference(b).size_hint(), (0, Some(fresh_cap(aw.len()))));
        assert_eq!(b.difference(a).size_hint(), (0, Some(fresh_cap(bw.len()))));
        assert_eq!(
            a.intersection(b).size_hint(),
            (0, Some(fresh_cap(aw.len().min(bw.len()))))
        );
        assert_eq!(
            b.intersection(a).size_hint(),
            (0, Some(fresh_cap(aw.len().min(bw.len()))))
        );
        assert_eq!(
            a.symmetric_difference(b).size_hint(),
            (0, Some(fresh_cap(aw.len().max(bw.len()))))
        );
        assert_eq!(
            b.symmetric_difference(a).size_hint(),
            (0, Some(fresh_cap(aw.len().max(bw.len()))))
        );
        assert_bounded_reducers(|| a.union(b));
        assert_bounded_reducers(|| b.union(a));
        assert_bounded_reducers(|| a.difference(b));
        assert_bounded_reducers(|| b.difference(a));
        assert_bounded_reducers(|| a.symmetric_difference(b));
        assert_bounded_reducers(|| b.symmetric_difference(a));
        assert_bounded_reducers(|| a.intersection(b));
        assert_bounded_reducers(|| b.intersection(a));
        assert_eq!(a.union(b).count(), manual_union(&aw, &bw));
        assert_eq!(b.union(a).count(), manual_union(&bw, &aw));
        assert_eq!(a.difference(b).count(), manual_difference(&aw, &bw));
        assert_eq!(b.difference(a).count(), manual_difference(&bw, &aw));
        assert_eq!(a.symmetric_difference(b).count(), manual_sym(&aw, &bw));
        assert_eq!(b.symmetric_difference(a).count(), manual_sym(&bw, &aw));
        assert_eq!(a.intersection(b).count(), manual_intersection(&aw, &bw));
        assert_eq!(b.intersection(a).count(), manual_intersection(&bw, &aw));
        assert_bounded_reducers(|| a.iter());
        assert_eq!(
            a.iter().count(),
            aw.iter()
                .map(|word| word.count_ones() as usize)
                .sum::<usize>()
        );
    }

    #[test]
    fn unequal_word_counts_match_elements_and_popcount() {
        let a3 = build::<3>(&[0, 1, 63, 64, 128]);
        let b1 = build::<1>(&[1, 2]);
        check_slice(&a3, &b1);

        let a2 = build::<2>(&[3, 70, 100]);
        let b4 = build::<4>(&[3, 70, 200, 255]);
        check_slice(&a2, &b4);

        let empty = build::<1>(&[]);
        let tail_only = build::<2>(&[70]);
        check_slice(&empty, &tail_only);
        check_slice(&build::<2>(&[]), &build::<4>(&[]));

        let eq_a = build::<2>(&[0, 5, 64]);
        let eq_b = build::<2>(&[5, 9, 70]);
        check_slice(&eq_a, &eq_b);

        let mut wide = BitSet::<[u32; 2], usize>::new();
        wide.insert(0);
        wide.insert(31);
        wide.insert(32);
        let mut narrow = BitSet::<[u32; 1], usize>::new();
        narrow.insert(5);
        narrow.insert(31);
        assert_eq!(wide.union(&narrow).collect::<Vec<_>>(), vec![0, 5, 31, 32]);
        assert_exact_reducers(|| wide.union(&narrow));
        assert_exact_reducers(|| narrow.union(&wide));
        assert_exact_reducers(|| wide.difference(&narrow));
        assert_exact_reducers(|| narrow.difference(&wide));
        assert_exact_reducers(|| wide.symmetric_difference(&narrow));
        assert_eq!(wide.difference(&narrow).collect::<Vec<_>>(), vec![0, 32]);
        assert_eq!(narrow.difference(&wide).collect::<Vec<_>>(), vec![5]);
        assert_eq!(
            wide.symmetric_difference(&narrow).collect::<Vec<_>>(),
            vec![0, 5, 32]
        );
    }

    #[test]
    fn unequal_values_are_ordered() {
        let a = build::<3>(&[0, 1, 63, 64, 128]);
        let b = build::<1>(&[1, 2]);
        assert_eq!(a.union(&b).collect::<Vec<_>>(), vec![0, 1, 2, 63, 64, 128]);
        assert_eq!(b.union(&a).collect::<Vec<_>>(), vec![0, 1, 2, 63, 64, 128]);
        assert_eq!(a.difference(&b).collect::<Vec<_>>(), vec![0, 63, 64, 128]);
        assert_eq!(b.difference(&a).collect::<Vec<_>>(), vec![2]);
        assert_eq!(a.intersection(&b).collect::<Vec<_>>(), vec![1]);
        assert_eq!(
            a.symmetric_difference(&b).collect::<Vec<_>>(),
            vec![0, 2, 63, 64, 128]
        );
        assert_eq!(
            b.symmetric_difference(&a).collect::<Vec<_>>(),
            vec![0, 2, 63, 64, 128]
        );
    }

    #[test]
    fn atomic_unequal_word_counts_and_iter() {
        let a3 = build_atomic::<3>(&[0, 1, 63, 64, 128]);
        let b1 = build_atomic::<1>(&[1, 2]);
        check_atomic(&a3, &b1);

        let a2 = build_atomic::<2>(&[3, 70]);
        let b4 = build_atomic::<4>(&[3, 200]);
        check_atomic(&a2, &b4);

        let empty = build_atomic::<1>(&[]);
        let tail_only = build_atomic::<2>(&[80]);
        check_atomic(&empty, &tail_only);

        let eq_a = build_atomic::<2>(&[0, 64]);
        let eq_b = build_atomic::<2>(&[1, 64]);
        check_atomic(&eq_a, &eq_b);

        let a3s: &AtomicBitSlice<AtomicU64, usize> = &a3;
        let b1s: &AtomicBitSlice<AtomicU64, usize> = &b1;
        assert_eq!(
            a3s.union(b1s).collect::<Vec<_>>(),
            vec![0, 1, 2, 63, 64, 128]
        );
        assert_eq!(
            a3s.difference(b1s).collect::<Vec<_>>(),
            vec![0, 63, 64, 128]
        );
        assert_eq!(
            a3s.symmetric_difference(b1s).collect::<Vec<_>>(),
            vec![0, 2, 63, 64, 128]
        );

        let small = AtomicBitSet::<[AtomicU32; 2], usize>::new();
        small.insert(0);
        small.insert(31);
        small.insert(40);
        assert_eq!(small.iter().collect::<Vec<_>>(), vec![0, 31, 40]);
        assert_eq!(small.iter().size_hint(), (0, Some(64)));
        let mut iter = small.iter();
        assert_eq!(iter.next(), Some(0));
        // Bit 31 is still in the loaded word; the next word is not loaded.
        assert_eq!(iter.size_hint(), (1, Some(1 + 32)));
        assert_bounded_hint(iter.size_hint(), 2);
        assert_eq!(iter.count(), 2);
        assert_bounded_reducers(|| small.iter());
        assert_eq!(small.iter().count(), small.len());
        assert_eq!(push_all(small.iter()), small.iter().collect::<Vec<_>>());
    }

    #[test]
    fn atomic_size_hint_ignores_unloaded_words() {
        let a = build_atomic::<2>(&[0, 1, 2, 64]);
        let b = build_atomic::<2>(&[]);
        let a_slice: &AtomicBitSlice<AtomicU64, usize> = &a;
        let b_slice: &AtomicBitSlice<AtomicU64, usize> = &b;
        let mut union = a_slice.union(b_slice);
        assert_eq!(union.size_hint(), (0, Some(128)));
        assert_eq!(union.next(), Some(0));
        assert_eq!(union.size_hint(), (2, Some(2 + 64)));
        assert_bounded_hint(union.size_hint(), 3);
        assert_eq!(push_all(union), vec![1, 2, 64]);

        let mut iter = a.iter();
        assert_eq!(iter.size_hint(), (0, Some(128)));
        assert_eq!(iter.next(), Some(0));
        assert_eq!(iter.size_hint(), (2, Some(66)));
        assert_eq!(iter.count(), 3);
        assert_eq!(a.iter().count(), push_all(a.iter()).len());
        assert_eq!(a.iter().collect::<Vec<_>>(), vec![0, 1, 2, 64]);

        let loads = Cell::new(0);
        let words_a = [AtomicU64::new(0b111), AtomicU64::new(1)];
        let words_b = [AtomicU64::new(0), AtomicU64::new(0)];
        let mut counted = super::WordOpIter::<AtomicU64, u64, usize, _, _, super::Bounded>::bounded(
            &words_a,
            &words_b,
            2,
            |left, right| left | right,
            |word: &AtomicU64| {
                loads.set(loads.get() + 1);
                word.load(Ordering::Relaxed)
            },
        );
        assert_eq!(counted.size_hint(), (0, Some(128)));
        assert_eq!(loads.get(), 0);
        assert_eq!(counted.next(), Some(0));
        let loads_after_next = loads.get();
        assert!(loads_after_next > 0);
        assert_eq!(counted.size_hint(), (2, Some(66)));
        assert_eq!(loads.get(), loads_after_next);
        assert_eq!(counted.count(), 3);

        let plain_loads = Cell::new(0);
        let plain_a = [0b111u64, 1];
        let plain_b = [0u64, 0];
        let exact = super::WordOpIter::<u64, u64, usize, _, _, super::Exact>::exact(
            &plain_a,
            &plain_b,
            2,
            |left, right| left | right,
            |word: &u64| {
                plain_loads.set(plain_loads.get() + 1);
                *word
            },
        );
        assert_eq!(exact.size_hint(), (4, Some(4)));
        assert!(plain_loads.get() > 0);
        assert_eq!(exact.count(), 4);
    }
}
