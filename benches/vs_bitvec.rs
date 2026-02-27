use std::hint::black_box;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use bitflagset::{AtomicBitSet, AtomicBoxedBitSet, BitSet, BoxedBitSet};
use bitvec::array::BitArray;
use bitvec::order::Lsb0;
use criterion::{Criterion, criterion_group, criterion_main};

// ── fixed-size benchmarks (BitSet<A, V> vs BitArray<A, Lsb0>) ──

macro_rules! bench_fixed {
    ($fn_name:ident, $group:literal, $bits:expr, $ours:ty, $bvec:ty, $probe:expr) => {
        fn $fn_name(c: &mut Criterion) {
            let bits_a: Vec<usize> = (0..$bits).step_by(3).collect();
            let bits_b: Vec<usize> = (0..$bits).step_by(5).collect();

            let mut ours_a = <$ours>::new();
            let mut ours_b = <$ours>::new();
            let mut bv_a = <$bvec>::ZERO;
            let mut bv_b = <$bvec>::ZERO;

            for &i in &bits_a {
                ours_a.insert(i);
                bv_a.set(i, true);
            }
            for &i in &bits_b {
                ours_b.insert(i);
                bv_b.set(i, true);
            }

            // sanity: both sides must agree
            let ours_len = ours_a.len();
            let bv_len = bv_a.count_ones();
            assert_eq!(
                ours_len, bv_len,
                "len mismatch: ours={ours_len} bitvec={bv_len}"
            );

            assert_eq!(ours_a.contains(&$probe), bv_a[$probe], "contains mismatch");
            assert_eq!(ours_a.is_empty(), bv_a.not_any(), "is_empty mismatch");

            let ours_iter_count: usize = ours_a.iter().count();
            let bv_iter_count = bv_a.iter_ones().count();
            assert_eq!(ours_iter_count, bv_iter_count, "iter count mismatch");

            let mut g = c.benchmark_group($group);

            // queries
            g.bench_function("ours/len", |b| b.iter(|| black_box(&ours_a).len()));
            g.bench_function("bitvec/count_ones", |b| {
                b.iter(|| black_box(&bv_a).count_ones())
            });

            g.bench_function("ours/is_empty", |b| {
                b.iter(|| black_box(&ours_a).is_empty())
            });
            g.bench_function("bitvec/not_any", |b| b.iter(|| black_box(&bv_a).not_any()));

            g.bench_function("ours/contains", |b| {
                b.iter(|| black_box(&ours_a).contains(black_box(&$probe)))
            });
            g.bench_function("bitvec/get", |b| {
                b.iter(|| black_box(&bv_a)[black_box($probe)])
            });

            // set relations
            g.bench_function("ours/is_subset", |b| {
                b.iter(|| black_box(&ours_b).is_subset(black_box(&ours_a)))
            });

            // binary ops
            g.bench_function("ours/bitor", |b| {
                b.iter(|| black_box(ours_a.clone()) | black_box(ours_b.clone()))
            });
            g.bench_function("bitvec/bitor", |b| {
                b.iter(|| black_box(bv_a) | black_box(bv_b))
            });

            g.bench_function("ours/bitand", |b| {
                b.iter(|| black_box(ours_a.clone()) & black_box(ours_b.clone()))
            });
            g.bench_function("bitvec/bitand", |b| {
                b.iter(|| black_box(bv_a) & black_box(bv_b))
            });

            g.bench_function("ours/bitxor", |b| {
                b.iter(|| black_box(ours_a.clone()) ^ black_box(ours_b.clone()))
            });
            g.bench_function("bitvec/bitxor", |b| {
                b.iter(|| black_box(bv_a) ^ black_box(bv_b))
            });

            g.bench_function("ours/not", |b| b.iter(|| !black_box(ours_a.clone())));
            g.bench_function("bitvec/not", |b| b.iter(|| !black_box(bv_a)));

            g.bench_function("ours/sub", |b| {
                b.iter(|| black_box(ours_a.clone()) - black_box(ours_b.clone()))
            });

            // mutation
            g.bench_function("ours/insert", |b| {
                b.iter(|| {
                    let mut s = ours_a.clone();
                    s.insert(black_box($probe));
                    black_box(&s);
                })
            });
            g.bench_function("bitvec/set_true", |b| {
                b.iter(|| {
                    let mut s = bv_a;
                    s.set(black_box($probe), true);
                    black_box(&s);
                })
            });

            g.bench_function("ours/remove", |b| {
                b.iter(|| {
                    let mut s = ours_a.clone();
                    s.remove(black_box($probe));
                    black_box(&s);
                })
            });
            g.bench_function("bitvec/set_false", |b| {
                b.iter(|| {
                    let mut s = bv_a;
                    s.set(black_box($probe), false);
                    black_box(&s);
                })
            });

            // iteration
            g.bench_function("ours/iter", |b| {
                b.iter(|| black_box(&ours_a).iter().count())
            });
            g.bench_function("bitvec/iter_ones", |b| {
                b.iter(|| black_box(&bv_a).iter_ones().count())
            });

            // drain
            g.bench_function("ours/drain", |b| {
                b.iter(|| {
                    let mut s = ours_a.clone();
                    black_box(s.drain().count());
                    debug_assert!(s.is_empty());
                })
            });
            g.bench_function("bitvec/drain_manual", |b| {
                b.iter(|| {
                    let mut s = bv_a;
                    let count = s.iter_ones().count();
                    s.fill(false);
                    black_box(count);
                })
            });

            g.finish();
        }
    };
}

bench_fixed!(bench_64, "64bit", 64, BitSet<u64, usize>, BitArray<u64, Lsb0>, 42);
bench_fixed!(bench_256, "256bit", 256, BitSet<[u64; 4], usize>, BitArray<[u64; 4], Lsb0>, 200);
bench_fixed!(bench_1024, "1024bit", 1024, BitSet<[u64; 16], usize>, BitArray<[u64; 16], Lsb0>, 800);

// ── heap-allocated benchmarks (BoxedBitSet vs bitvec BitVec) ────

fn bench_boxed(c: &mut Criterion) {
    const BITS: usize = 65536;
    let probe: usize = 50000;

    let mut ours_a = BoxedBitSet::<u64, usize>::with_capacity(BITS);
    let mut ours_b = BoxedBitSet::<u64, usize>::with_capacity(BITS);
    let mut bv_a = bitvec::vec::BitVec::<u64, Lsb0>::repeat(false, BITS);
    let mut bv_b = bitvec::vec::BitVec::<u64, Lsb0>::repeat(false, BITS);

    for i in (0..BITS).step_by(3) {
        ours_a.insert(i);
        bv_a.set(i, true);
    }
    for i in (0..BITS).step_by(5) {
        ours_b.insert(i);
        bv_b.set(i, true);
    }

    // sanity: both sides must agree
    let ours_len = ours_a.len();
    let bv_len = bv_a.count_ones();
    assert_eq!(
        ours_len, bv_len,
        "len mismatch: ours={ours_len} bitvec={bv_len}"
    );

    let ours_len_b = ours_b.len();
    let bv_len_b = bv_b.count_ones();
    assert_eq!(
        ours_len_b, bv_len_b,
        "len_b mismatch: ours={ours_len_b} bitvec={bv_len_b}"
    );

    assert_eq!(
        ours_a.contains(&probe),
        bv_a[probe],
        "contains mismatch at {probe}"
    );
    assert_eq!(ours_a.is_empty(), bv_a.not_any(), "is_empty mismatch");

    // spot-check iteration count
    let ours_iter_count: usize = ours_a.iter().count();
    let bv_iter_count = bv_a.iter_ones().count();
    assert_eq!(ours_iter_count, bv_iter_count, "iter count mismatch");

    let mut g = c.benchmark_group("65536bit_boxed");

    // queries
    g.bench_function("ours/len", |b| b.iter(|| black_box(&*ours_a).len()));
    g.bench_function("bitvec/count_ones", |b| {
        b.iter(|| black_box(&bv_a).count_ones())
    });

    g.bench_function("ours/is_empty", |b| {
        b.iter(|| black_box(&*ours_a).is_empty())
    });
    g.bench_function("bitvec/not_any", |b| b.iter(|| black_box(&bv_a).not_any()));

    g.bench_function("ours/contains", |b| {
        b.iter(|| black_box(&*ours_a).contains(black_box(&probe)))
    });
    g.bench_function("bitvec/get", |b| {
        b.iter(|| black_box(&bv_a)[black_box(probe)])
    });

    // mutation
    g.bench_function("ours/insert", |b| {
        b.iter(|| {
            let mut s = BoxedBitSet::<u64, usize>::with_capacity(BITS);
            s.insert(black_box(probe));
            black_box(&s);
        })
    });
    g.bench_function("bitvec/set_true", |b| {
        b.iter(|| {
            let mut s = bitvec::vec::BitVec::<u64, Lsb0>::repeat(false, BITS);
            s.set(black_box(probe), true);
            black_box(&s);
        })
    });

    // iteration
    g.bench_function("ours/iter", |b| {
        b.iter(|| black_box(&*ours_a).iter().count())
    });
    g.bench_function("bitvec/iter_ones", |b| {
        b.iter(|| black_box(&bv_a).iter_ones().count())
    });

    // drain
    g.bench_function("ours/drain", |b| {
        b.iter(|| {
            let mut s = ours_a.clone();
            black_box(s.drain().count());
            debug_assert!(s.is_empty());
        })
    });
    g.bench_function("bitvec/drain_manual", |b| {
        b.iter(|| {
            let mut s = bv_a.clone();
            let count = s.iter_ones().count();
            s.fill(false);
            black_box(count);
        })
    });

    g.finish();
}

// ── atomic fixed-size benchmarks (AtomicBitSet vs bitvec atomic BitArray) ──

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
    let bv = bitvec::vec::BitVec::<AtomicU64, Lsb0>::repeat(false, BITS);

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

    g.bench_function("atomic/insert", |b| {
        b.iter(|| {
            let s = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(BITS);
            s.insert(black_box(probe));
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
