use std::hint::black_box;
use std::time::Duration;

use bitflagset::{BitSet, BoxedBitSet};
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

// `bit_set::BitSet` (default block `u32`) is heap-backed. It is compared with
// fixed `BitSet<[u64; N], usize>` and with `BoxedBitSet<u64, usize>`, both
// pre-sized to the same bit capacity (`with_capacity` / `from_elem`) so growth
// is not part of the measurement.
//
// `bit_vec::BitVec` (default block `u32`) is only used for word-level ops:
// `set` / `get`, in-place `or` / `and` / `difference`, `count_ones`, `none`,
// and `fill(false)`.
//
// Skipped, because the other crate has no equivalent:
// - `first` / `last` — neither `bit_set::BitSet` nor `bit_vec::BitVec`.
// - Owned `|` / `&` / `-` that return a new set. `bit-set` exposes lazy
//   iterators (`union` / `intersection` / `difference`) plus in-place
//   `*_with`. `bit-vec` only mutates in place (`or` / `and` / `difference`).
//   Cloning and then mutating would measure allocation, so it is not used as
//   a stand-in.
//
// Mutating rows (`insert`, `remove`, `set`, `set_false`, `clear`, and the
// in-place ops) clone the destination in `iter_batched_ref` setup. `&=` and
// `-=` also clone the right-hand side there, because those operators take it
// by value (`iter_batched`). The timed closure is only the operation.
// Read-only rows stay on `iter`. Fixed-size groups use `SmallInput`. The
// 65536-bit group uses `LargeInput` so a batch of heap clones stays modest.
// - `bit_vec::BitVec::insert` / `remove` — those shift storage. Membership
//   writes go through `set`.
// - Set-index iteration on `bit-vec` — `iter` yields a `bool` per index, not
//   the indices of set bits.
// - `bit_set::BitSet::len` and `clear`, and `bit_vec::BitVec::clear`, are
//   deprecated aliases. The benches call `count`, `make_empty`, and
//   `fill(false)`.

#[inline]
fn dup<T: Clone>(value: &T) -> T {
    value.clone()
}

macro_rules! bench_vs_bit_set {
    (
        $fn_name:ident,
        $group:literal,
        $bits:expr,
        $ours:ty,
        $probe:expr,
        $empty:expr,
        $batch:expr
    ) => {
        fn $fn_name(c: &mut Criterion) {
            let probe: usize = $probe;

            let mut ours_a: $ours = $empty;
            let mut ours_b: $ours = $empty;
            let mut bs_a = bit_set::BitSet::with_capacity($bits);
            let mut bs_b = bit_set::BitSet::with_capacity($bits);
            let mut bv_a = bit_vec::BitVec::from_elem($bits, false);
            let mut bv_b = bit_vec::BitVec::from_elem($bits, false);

            for i in (0..$bits).step_by(3) {
                ours_a.insert(i);
                bs_a.insert(i);
                bv_a.set(i, true);
            }
            for i in (0..$bits).step_by(5) {
                ours_b.insert(i);
                bs_b.insert(i);
                bv_b.set(i, true);
            }

            assert_eq!(ours_a.len(), bs_a.count(), "len mismatch vs bit-set");
            assert_eq!(
                ours_a.len() as u64,
                bv_a.count_ones(),
                "len mismatch vs bit-vec"
            );
            assert_eq!(ours_b.len(), bs_b.count(), "len_b mismatch vs bit-set");
            assert_eq!(
                ours_a.contains(&probe),
                bs_a.contains(probe),
                "contains mismatch vs bit-set"
            );
            assert_eq!(
                ours_a.contains(&probe),
                bv_a.get(probe) == Some(true),
                "contains mismatch vs bit-vec"
            );
            assert_eq!(ours_a.is_empty(), bs_a.is_empty(), "is_empty mismatch");
            assert_eq!(ours_a.is_empty(), bv_a.none(), "none mismatch");
            assert_eq!(ours_a.iter().count(), bs_a.iter().count(), "iter mismatch");
            assert_eq!(
                ours_a.union(&ours_b).count(),
                bs_a.union(&bs_b).count(),
                "union mismatch"
            );
            assert_eq!(
                ours_a.intersection(&ours_b).count(),
                bs_a.intersection(&bs_b).count(),
                "intersection mismatch"
            );
            assert_eq!(
                ours_a.difference(&ours_b).count(),
                bs_a.difference(&bs_b).count(),
                "difference mismatch"
            );
            assert_eq!(
                ours_b.is_subset(&ours_a),
                bs_b.is_subset(&bs_a),
                "is_subset mismatch"
            );

            let mut united = dup(&ours_a);
            united.union_from(&ours_b);
            let mut united_bs = dup(&bs_a);
            united_bs.union_with(&bs_b);
            let mut united_bv = dup(&bv_a);
            united_bv.or(&bv_b);
            assert_eq!(united.len(), united_bs.count(), "union_with mismatch");
            assert_eq!(united.len() as u64, united_bv.count_ones(), "or mismatch");

            let mut intersected = dup(&ours_a);
            intersected &= dup(&ours_b);
            let mut intersected_bs = dup(&bs_a);
            intersected_bs.intersect_with(&bs_b);
            let mut intersected_bv = dup(&bv_a);
            intersected_bv.and(&bv_b);
            assert_eq!(
                intersected.len(),
                intersected_bs.count(),
                "intersect_with mismatch"
            );
            assert_eq!(
                intersected.len() as u64,
                intersected_bv.count_ones(),
                "and mismatch"
            );

            let mut differenced = dup(&ours_a);
            differenced -= dup(&ours_b);
            let mut differenced_bs = dup(&bs_a);
            differenced_bs.difference_with(&bs_b);
            let mut differenced_bv = dup(&bv_a);
            differenced_bv.difference(&bv_b);
            assert_eq!(
                differenced.len(),
                differenced_bs.count(),
                "difference_with mismatch"
            );
            assert_eq!(
                differenced.len() as u64,
                differenced_bv.count_ones(),
                "bit-vec difference mismatch"
            );

            let mut g = c.benchmark_group($group);

            g.bench_function("ours/len", |b| b.iter(|| black_box(&ours_a).len()));
            g.bench_function("bitset/count", |b| b.iter(|| black_box(&bs_a).count()));
            g.bench_function("bitvec/count_ones", |b| {
                b.iter(|| black_box(&bv_a).count_ones())
            });

            g.bench_function("ours/is_empty", |b| {
                b.iter(|| black_box(&ours_a).is_empty())
            });
            g.bench_function("bitset/is_empty", |b| {
                b.iter(|| black_box(&bs_a).is_empty())
            });
            g.bench_function("bitvec/none", |b| b.iter(|| black_box(&bv_a).none()));

            g.bench_function("ours/contains", |b| {
                b.iter(|| black_box(&ours_a).contains(black_box(&probe)))
            });
            g.bench_function("bitset/contains", |b| {
                b.iter(|| black_box(&bs_a).contains(black_box(probe)))
            });
            g.bench_function("bitvec/get", |b| {
                b.iter(|| black_box(&bv_a).get(black_box(probe)))
            });

            g.bench_function("ours/insert", |b| {
                b.iter_batched_ref(
                    || dup(&ours_a),
                    |s| {
                        s.insert(black_box(probe));
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitset/insert", |b| {
                b.iter_batched_ref(
                    || dup(&bs_a),
                    |s| {
                        s.insert(black_box(probe));
                        black_box(s);
                    },
                    $batch,
                );
            });

            g.bench_function("ours/remove", |b| {
                b.iter_batched_ref(
                    || dup(&ours_a),
                    |s| {
                        s.remove(black_box(probe));
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitset/remove", |b| {
                b.iter_batched_ref(
                    || dup(&bs_a),
                    |s| {
                        s.remove(black_box(probe));
                        black_box(s);
                    },
                    $batch,
                );
            });

            g.bench_function("ours/set", |b| {
                b.iter_batched_ref(
                    || dup(&ours_a),
                    |s| {
                        s.set(black_box(probe), true);
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitvec/set", |b| {
                b.iter_batched_ref(
                    || dup(&bv_a),
                    |s| {
                        s.set(black_box(probe), true);
                        black_box(s);
                    },
                    $batch,
                );
            });

            g.bench_function("ours/set_false", |b| {
                b.iter_batched_ref(
                    || dup(&ours_a),
                    |s| {
                        s.set(black_box(probe), false);
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitvec/set_false", |b| {
                b.iter_batched_ref(
                    || dup(&bv_a),
                    |s| {
                        s.set(black_box(probe), false);
                        black_box(s);
                    },
                    $batch,
                );
            });

            g.bench_function("ours/iter", |b| {
                b.iter(|| black_box(&ours_a).iter().count())
            });
            g.bench_function("bitset/iter", |b| {
                b.iter(|| black_box(&bs_a).iter().count())
            });

            g.bench_function("ours/union", |b| {
                b.iter(|| black_box(&ours_a).union(black_box(&ours_b)).count())
            });
            g.bench_function("bitset/union", |b| {
                b.iter(|| black_box(&bs_a).union(black_box(&bs_b)).count())
            });

            g.bench_function("ours/intersection", |b| {
                b.iter(|| black_box(&ours_a).intersection(black_box(&ours_b)).count())
            });
            g.bench_function("bitset/intersection", |b| {
                b.iter(|| black_box(&bs_a).intersection(black_box(&bs_b)).count())
            });

            g.bench_function("ours/difference", |b| {
                b.iter(|| black_box(&ours_a).difference(black_box(&ours_b)).count())
            });
            g.bench_function("bitset/difference", |b| {
                b.iter(|| black_box(&bs_a).difference(black_box(&bs_b)).count())
            });

            g.bench_function("ours/union_from", |b| {
                b.iter_batched_ref(
                    || dup(&ours_a),
                    |s| {
                        s.union_from(black_box(&ours_b));
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitset/union_with", |b| {
                b.iter_batched_ref(
                    || dup(&bs_a),
                    |s| {
                        s.union_with(black_box(&bs_b));
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitvec/or", |b| {
                b.iter_batched_ref(
                    || dup(&bv_a),
                    |s| {
                        black_box(s.or(black_box(&bv_b)));
                        black_box(s);
                    },
                    $batch,
                );
            });

            g.bench_function("ours/intersect_assign", |b| {
                b.iter_batched(
                    || (dup(&ours_a), dup(&ours_b)),
                    |(mut s, rhs)| {
                        s &= black_box(rhs);
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitset/intersect_with", |b| {
                b.iter_batched_ref(
                    || dup(&bs_a),
                    |s| {
                        s.intersect_with(black_box(&bs_b));
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitvec/and", |b| {
                b.iter_batched_ref(
                    || dup(&bv_a),
                    |s| {
                        black_box(s.and(black_box(&bv_b)));
                        black_box(s);
                    },
                    $batch,
                );
            });

            g.bench_function("ours/difference_assign", |b| {
                b.iter_batched(
                    || (dup(&ours_a), dup(&ours_b)),
                    |(mut s, rhs)| {
                        s -= black_box(rhs);
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitset/difference_with", |b| {
                b.iter_batched_ref(
                    || dup(&bs_a),
                    |s| {
                        s.difference_with(black_box(&bs_b));
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitvec/difference", |b| {
                b.iter_batched_ref(
                    || dup(&bv_a),
                    |s| {
                        black_box(s.difference(black_box(&bv_b)));
                        black_box(s);
                    },
                    $batch,
                );
            });

            g.bench_function("ours/is_subset", |b| {
                b.iter(|| black_box(&ours_b).is_subset(black_box(&ours_a)))
            });
            g.bench_function("bitset/is_subset", |b| {
                b.iter(|| black_box(&bs_b).is_subset(black_box(&bs_a)))
            });

            g.bench_function("ours/clear", |b| {
                b.iter_batched_ref(
                    || dup(&ours_a),
                    |s| {
                        s.clear();
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitset/make_empty", |b| {
                b.iter_batched_ref(
                    || dup(&bs_a),
                    |s| {
                        s.make_empty();
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitvec/fill", |b| {
                b.iter_batched_ref(
                    || dup(&bv_a),
                    |s| {
                        s.fill(false);
                        black_box(s);
                    },
                    $batch,
                );
            });

            g.finish();
        }
    };
}

bench_vs_bit_set!(
    bench_64,
    "64bit_vs_bit_set",
    64,
    BitSet<[u64; 1], usize>,
    42,
    BitSet::<[u64; 1], usize>::new(),
    BatchSize::SmallInput
);
bench_vs_bit_set!(
    bench_256,
    "256bit_vs_bit_set",
    256,
    BitSet<[u64; 4], usize>,
    200,
    BitSet::<[u64; 4], usize>::new(),
    BatchSize::SmallInput
);
bench_vs_bit_set!(
    bench_1024,
    "1024bit_vs_bit_set",
    1024,
    BitSet<[u64; 16], usize>,
    800,
    BitSet::<[u64; 16], usize>::new(),
    BatchSize::SmallInput
);
bench_vs_bit_set!(
    bench_boxed,
    "65536bit_boxed_vs_bit_set",
    65536,
    BoxedBitSet<u64, usize>,
    50000,
    BoxedBitSet::<u64, usize>::with_capacity(65536),
    BatchSize::LargeInput
);

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    targets = bench_64, bench_256, bench_1024, bench_boxed,
}
criterion_main!(benches);
