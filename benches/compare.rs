use std::hint::black_box;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use bitflagset::{AtomicBitSet, AtomicBoxedBitSet, BitSet, BoxedBitSet};
use bitvec::array::BitArray;
use bitvec::order::Lsb0;
use bitvec::vec::BitVec;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

// One Criterion group per size. That group measures bitflagset once per
// operation, next to bitvec (`BitArray` / `BitVec<u64, Lsb0>`), `bit_set::BitSet`
// (default `u32` blocks, `with_capacity`), and `bit_vec::BitVec` (default `u32`
// blocks, `from_elem`).
//
// `bit_vec::BitVec` is only used for word-level ops: `set` / `get`, in-place
// `or` / `and` / `difference`, `count_ones`, `none`, and `fill(false)`.
//
// Skipped, because the other crate has no equivalent:
// - `first` / `last` — neither `bit_set::BitSet` nor `bit_vec::BitVec`.
// - Owned `|` / `&` / `-` on `bit-set` / `bit-vec`. Those producing operators
//   are the bitvec `bitor` / `bitand` / `bitxor` / `not` rows. Cloning and then
//   mutating would measure allocation, so it is not used as a stand-in.
//
// Mutating rows (`insert`, `remove`, `set`, `set_false`, `clear`, and the
// in-place ops) clone the destination in `iter_batched_ref` setup. In-place
// `union_from` / `intersect_from` / `difference_from` borrow the right-hand
// side, matching `union_with` / `intersect_with` / `difference_with` and
// bit-vec `or` / `and` / `difference`. The timed closure is only the operation.
// Read-only rows stay on `iter`. By-value `bitor` / `bitand` / `bitxor` / `not`
// clone inside the timed closure, because the operator consumes both sets.
// Fixed-size groups use `SmallInput`. The 65536-bit group uses `LargeInput`.
// - `bit_vec::BitVec::insert` / `remove` shift storage. Membership writes go
//   through `set`.
// - Set-index iteration on `bit-vec` — `iter` yields a `bool` per index.
// - `bit_set::BitSet::len` and `clear`, and `bit_vec::BitVec::clear`, are
//   deprecated aliases. The benches call `count`, `make_empty`, and `fill(false)`.

#[inline]
fn dup<T: Clone>(value: &T) -> T {
    value.clone()
}

macro_rules! bench_all {
    (
        $fn_name:ident,
        $group:literal,
        $bits:expr,
        $ours:ty,
        $bitarr:ty,
        $probe:expr,
        $empty:expr,
        $bitarr_empty:expr,
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
            let mut ba_a: $bitarr = $bitarr_empty;
            let mut ba_b: $bitarr = $bitarr_empty;

            for i in (0..$bits).step_by(3) {
                ours_a.insert(i);
                bs_a.insert(i);
                bv_a.set(i, true);
                ba_a.set(i, true);
            }
            for i in (0..$bits).step_by(5) {
                ours_b.insert(i);
                bs_b.insert(i);
                bv_b.set(i, true);
                ba_b.set(i, true);
            }

            assert_eq!(ours_a.len(), bs_a.count(), "len mismatch vs bit-set");
            assert_eq!(
                ours_a.len() as u64,
                bv_a.count_ones(),
                "len mismatch vs bit-vec"
            );
            assert_eq!(ours_a.len(), ba_a.count_ones(), "len mismatch vs bitvec");
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
            assert_eq!(
                ours_a.contains(&probe),
                ba_a[probe],
                "contains mismatch vs bitvec"
            );
            assert_eq!(ours_a.is_empty(), bs_a.is_empty(), "is_empty mismatch");
            assert_eq!(ours_a.is_empty(), bv_a.none(), "none mismatch");
            assert_eq!(ours_a.iter().count(), bs_a.iter().count(), "iter mismatch");
            assert_eq!(
                ours_a.iter().count(),
                ba_a.iter_ones().count(),
                "iter mismatch vs bitvec"
            );
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
            assert_eq!(
                (ours_a.clone() | ours_b.clone()).len(),
                (ba_a.clone() | ba_b.clone()).count_ones(),
                "bitor mismatch"
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
            intersected.intersect_from(&ours_b);
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
            differenced.difference_from(&ours_b);
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
            g.bench_function("bit_vec/count_ones", |b| {
                b.iter(|| black_box(&bv_a).count_ones())
            });
            g.bench_function("bitvec/count_ones", |b| {
                b.iter(|| black_box(&ba_a).count_ones())
            });

            g.bench_function("ours/contains", |b| {
                b.iter(|| black_box(&ours_a).contains(black_box(&probe)))
            });
            g.bench_function("bitset/contains", |b| {
                b.iter(|| black_box(&bs_a).contains(black_box(probe)))
            });
            g.bench_function("bit_vec/get", |b| {
                b.iter(|| black_box(&bv_a).get(black_box(probe)))
            });
            g.bench_function("bitvec/get", |b| {
                b.iter(|| black_box(&ba_a)[black_box(probe)])
            });

            g.bench_function("ours/index", |b| {
                b.iter(|| black_box(&ours_a)[black_box(probe)])
            });
            g.bench_function("bitvec/index", |b| {
                b.iter(|| black_box(&ba_a)[black_box(probe)])
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

            // `probe` is clear in every group, including 64-bit. `present_bit`
            // is the nearest lower multiple of 3, which the setup loop sets.
            let present_bit: usize = probe / 3 * 3;
            assert_ne!(probe, present_bit, "absent and present probes must differ");
            assert!(
                ours_a.contains(&present_bit),
                "present bit {present_bit} must be set"
            );
            assert!(bs_a.contains(present_bit), "present bit missing on bit-set");
            assert_eq!(
                bv_a.get(present_bit),
                Some(true),
                "present bit missing on bit-vec"
            );
            assert!(ba_a[present_bit], "present bit missing on bitvec");
            assert!(
                !ours_a.contains(&probe),
                "insert/set/absent rows require a clear probe"
            );
            assert!(!bs_a.contains(probe), "absent probe is set on bit-set");
            assert_eq!(
                bv_a.get(probe),
                Some(false),
                "absent probe is set on bit-vec"
            );
            assert!(!ba_a[probe], "absent probe is set on bitvec");

            g.bench_function("ours/remove_present", |b| {
                b.iter_batched_ref(
                    || dup(&ours_a),
                    |s| {
                        s.remove(black_box(present_bit));
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bitset/remove_present", |b| {
                b.iter_batched_ref(
                    || dup(&bs_a),
                    |s| {
                        s.remove(black_box(present_bit));
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
            g.bench_function("bit_vec/set", |b| {
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
            g.bench_function("bit_vec/set_false", |b| {
                b.iter_batched_ref(
                    || dup(&bv_a),
                    |s| {
                        s.set(black_box(probe), false);
                        black_box(s);
                    },
                    $batch,
                );
            });

            g.bench_function("ours/set_false_present", |b| {
                b.iter_batched_ref(
                    || dup(&ours_a),
                    |s| {
                        s.set(black_box(present_bit), false);
                        black_box(s);
                    },
                    $batch,
                );
            });
            g.bench_function("bit_vec/set_false_present", |b| {
                b.iter_batched_ref(
                    || dup(&bv_a),
                    |s| {
                        s.set(black_box(present_bit), false);
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
            g.bench_function("bitvec/iter_ones", |b| {
                b.iter(|| black_box(&ba_a).iter_ones().count())
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
            g.bench_function("bit_vec/or", |b| {
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
                b.iter_batched_ref(
                    || dup(&ours_a),
                    |s| {
                        s.intersect_from(black_box(&ours_b));
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
            g.bench_function("bit_vec/and", |b| {
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
                b.iter_batched_ref(
                    || dup(&ours_a),
                    |s| {
                        s.difference_from(black_box(&ours_b));
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
            g.bench_function("bit_vec/difference", |b| {
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
            g.bench_function("bit_vec/fill", |b| {
                b.iter_batched_ref(
                    || dup(&bv_a),
                    |s| {
                        s.fill(false);
                        black_box(s);
                    },
                    $batch,
                );
            });

            g.bench_function("ours/bitor", |b| {
                b.iter(|| black_box(ours_a.clone()) | black_box(ours_b.clone()))
            });
            g.bench_function("bitvec/bitor", |b| {
                b.iter(|| black_box(ba_a.clone()) | black_box(ba_b.clone()))
            });

            g.bench_function("ours/bitand", |b| {
                b.iter(|| black_box(ours_a.clone()) & black_box(ours_b.clone()))
            });
            g.bench_function("bitvec/bitand", |b| {
                b.iter(|| black_box(ba_a.clone()) & black_box(ba_b.clone()))
            });

            g.bench_function("ours/bitxor", |b| {
                b.iter(|| black_box(ours_a.clone()) ^ black_box(ours_b.clone()))
            });
            g.bench_function("bitvec/bitxor", |b| {
                b.iter(|| black_box(ba_a.clone()) ^ black_box(ba_b.clone()))
            });

            g.bench_function("ours/not", |b| b.iter(|| !black_box(ours_a.clone())));
            g.bench_function("bitvec/not", |b| b.iter(|| !black_box(ba_a.clone())));

            g.finish();
        }
    };
}

bench_all!(
    bench_64,
    "64bit",
    64,
    BitSet<[u64; 1], usize>,
    BitArray<[u64; 1], Lsb0>,
    41,
    BitSet::<[u64; 1], usize>::new(),
    BitArray::<[u64; 1], Lsb0>::ZERO,
    BatchSize::SmallInput
);
bench_all!(
    bench_256,
    "256bit",
    256,
    BitSet<[u64; 4], usize>,
    BitArray<[u64; 4], Lsb0>,
    200,
    BitSet::<[u64; 4], usize>::new(),
    BitArray::<[u64; 4], Lsb0>::ZERO,
    BatchSize::SmallInput
);
bench_all!(
    bench_1024,
    "1024bit",
    1024,
    BitSet<[u64; 16], usize>,
    BitArray<[u64; 16], Lsb0>,
    800,
    BitSet::<[u64; 16], usize>::new(),
    BitArray::<[u64; 16], Lsb0>::ZERO,
    BatchSize::SmallInput
);
bench_all!(
    bench_boxed,
    "65536bit_boxed",
    65536,
    BoxedBitSet<u64, usize>,
    BitVec<u64, Lsb0>,
    50000,
    BoxedBitSet::<u64, usize>::with_capacity(65536),
    BitVec::<u64, Lsb0>::repeat(false, 65536),
    BatchSize::LargeInput
);

// ── atomic (one group per size, bitflagset vs bitvec) ──────────

macro_rules! bench_atomic_fixed {
    ($fn_name:ident, $group:literal, $bits:expr, $n:expr, $probe:expr) => {
        fn $fn_name(c: &mut Criterion) {
            let bits_a: Vec<usize> = (0..$bits).step_by(3).collect();

            let atomic_a = AtomicBitSet::<[AtomicU64; $n], usize>::new();
            let bv_a =
                BitArray::<[AtomicU64; $n], Lsb0>::new(core::array::from_fn(|_| AtomicU64::new(0)));

            for &i in &bits_a {
                atomic_a.insert(i);
                bv_a.as_bitslice().set_aliased(i, true);
            }

            assert_eq!(atomic_a.len(), bv_a.count_ones(), "len mismatch");
            assert_eq!(
                atomic_a.contains(&$probe),
                *bv_a.get($probe).unwrap(),
                "contains mismatch"
            );

            let mut g = c.benchmark_group($group);

            g.bench_function("atomic/len", |b| b.iter(|| black_box(&atomic_a).len()));
            g.bench_function("bitvec/count_ones", |b| {
                b.iter(|| black_box(&bv_a).count_ones())
            });

            g.bench_function("atomic/is_empty", |b| {
                b.iter(|| black_box(&atomic_a).is_empty())
            });
            g.bench_function("bitvec/not_any", |b| b.iter(|| black_box(&bv_a).not_any()));

            g.bench_function("atomic/contains", |b| {
                b.iter(|| black_box(&atomic_a).contains(black_box(&$probe)))
            });
            g.bench_function("bitvec/get", |b| {
                b.iter(|| *black_box(&bv_a).get(black_box($probe)).unwrap())
            });

            g.bench_function("atomic/index", |b| {
                b.iter(|| black_box(&atomic_a)[black_box($probe)])
            });
            g.bench_function("bitvec/index", |b| {
                b.iter(|| *black_box(&bv_a).get(black_box($probe)).unwrap())
            });

            g.bench_function("atomic/insert", |b| {
                b.iter(|| {
                    let s = AtomicBitSet::<[AtomicU64; $n], usize>::new();
                    s.insert(black_box($probe));
                    black_box(&s);
                })
            });
            g.bench_function("bitvec/set_aliased", |b| {
                b.iter(|| {
                    let s = BitArray::<[AtomicU64; $n], Lsb0>::new(core::array::from_fn(|_| {
                        AtomicU64::new(0)
                    }));
                    s.as_bitslice().set_aliased(black_box($probe), true);
                    black_box(&s);
                })
            });

            g.bench_function("atomic/iter", |b| {
                b.iter(|| black_box(&atomic_a).iter().count())
            });
            g.bench_function("bitvec/iter_ones", |b| {
                b.iter(|| black_box(&bv_a).iter_ones().count())
            });

            g.finish();
        }
    };
}

fn bench_atomic_64_vs_bitvec(c: &mut Criterion) {
    let probe: usize = 42;

    let atomic_a = AtomicBitSet::<AtomicU64, usize>::new();
    let bv_a = BitArray::<AtomicU64, Lsb0>::new(AtomicU64::new(0));

    for i in (0..64).step_by(3) {
        atomic_a.insert(i);
        bv_a.as_bitslice().set_aliased(i, true);
    }

    assert_eq!(atomic_a.len(), bv_a.count_ones(), "len mismatch");

    let mut g = c.benchmark_group("64bit_atomic_vs_bitvec");

    g.bench_function("atomic/len", |b| b.iter(|| black_box(&atomic_a).len()));
    g.bench_function("bitvec/count_ones", |b| {
        b.iter(|| black_box(&bv_a).count_ones())
    });

    g.bench_function("atomic/is_empty", |b| {
        b.iter(|| black_box(&atomic_a).is_empty())
    });
    g.bench_function("bitvec/not_any", |b| b.iter(|| black_box(&bv_a).not_any()));

    g.bench_function("atomic/contains", |b| {
        b.iter(|| black_box(&atomic_a).contains(black_box(&probe)))
    });
    g.bench_function("bitvec/get", |b| {
        b.iter(|| *black_box(&bv_a).get(black_box(probe)).unwrap())
    });

    g.bench_function("atomic/index", |b| {
        b.iter(|| black_box(&atomic_a)[black_box(probe)])
    });
    g.bench_function("bitvec/index", |b| {
        b.iter(|| *black_box(&bv_a).get(black_box(probe)).unwrap())
    });

    g.bench_function("atomic/insert", |b| {
        b.iter(|| {
            let s = AtomicBitSet::<AtomicU64, usize>::new();
            s.insert(black_box(probe));
            black_box(&s);
        })
    });
    g.bench_function("bitvec/set_aliased", |b| {
        b.iter(|| {
            let s = BitArray::<AtomicU64, Lsb0>::new(AtomicU64::new(0));
            s.as_bitslice().set_aliased(black_box(probe), true);
            black_box(&s);
        })
    });

    g.bench_function("atomic/iter", |b| {
        b.iter(|| black_box(&atomic_a).iter().count())
    });
    g.bench_function("bitvec/iter_ones", |b| {
        b.iter(|| black_box(&bv_a).iter_ones().count())
    });

    g.finish();
}

bench_atomic_fixed!(
    bench_atomic_256_vs_bitvec,
    "256bit_atomic_vs_bitvec",
    256,
    4,
    200
);
bench_atomic_fixed!(
    bench_atomic_1024_vs_bitvec,
    "1024bit_atomic_vs_bitvec",
    1024,
    16,
    800
);

fn bench_atomic_boxed_vs_bitvec(c: &mut Criterion) {
    const BITS: usize = 65536;
    let probe: usize = 50000;

    let atomic = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(BITS);
    let bv = BitVec::<AtomicU64, Lsb0>::repeat(false, BITS);

    for i in (0..BITS).step_by(3) {
        atomic.insert(i);
        bv.as_bitslice().set_aliased(i, true);
    }

    assert_eq!(atomic.len(), bv.count_ones(), "len mismatch");

    let mut g = c.benchmark_group("65536bit_atomic_boxed_vs_bitvec");

    g.bench_function("atomic/len", |b| b.iter(|| black_box(&*atomic).len()));
    g.bench_function("bitvec/count_ones", |b| {
        b.iter(|| black_box(&bv).count_ones())
    });

    g.bench_function("atomic/is_empty", |b| {
        b.iter(|| black_box(&*atomic).is_empty())
    });
    g.bench_function("bitvec/not_any", |b| b.iter(|| black_box(&bv).not_any()));

    g.bench_function("atomic/contains", |b| {
        b.iter(|| black_box(&*atomic).contains(black_box(&probe)))
    });
    g.bench_function("bitvec/get", |b| {
        b.iter(|| *black_box(&bv).get(black_box(probe)).unwrap())
    });

    g.bench_function("atomic/index", |b| {
        b.iter(|| black_box(&*atomic)[black_box(probe)])
    });
    g.bench_function("bitvec/index", |b| {
        b.iter(|| *black_box(&bv).get(black_box(probe)).unwrap())
    });

    g.bench_function("atomic/insert", |b| {
        b.iter(|| {
            let s = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(BITS);
            s.insert(black_box(probe));
            black_box(&s);
        })
    });
    g.bench_function("bitvec/set_aliased", |b| {
        b.iter(|| {
            let s = BitVec::<AtomicU64, Lsb0>::repeat(false, BITS);
            s.as_bitslice().set_aliased(black_box(probe), true);
            black_box(&s);
        })
    });

    g.bench_function("atomic/iter", |b| {
        b.iter(|| black_box(&*atomic).iter().count())
    });
    g.bench_function("bitvec/iter_ones", |b| {
        b.iter(|| black_box(&bv).iter_ones().count())
    });

    g.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    targets =
        bench_64,
        bench_256,
        bench_1024,
        bench_boxed,
        bench_atomic_64_vs_bitvec,
        bench_atomic_256_vs_bitvec,
        bench_atomic_1024_vs_bitvec,
        bench_atomic_boxed_vs_bitvec,
}
criterion_main!(benches);
