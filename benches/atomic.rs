use std::hint::black_box;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use bitflagset::{AtomicBitSet, AtomicBoxedBitSet, BitSet, BoxedBitSet};
use criterion::{Criterion, criterion_group, criterion_main};

// ── single prim: AtomicBitSet<AtomicU64> vs BitSet<u64> ─────────

fn bench_atomic_64(c: &mut Criterion) {
    let probe: usize = 42;

    let mut plain = BitSet::<u64, usize>::new();
    for i in (0..64).step_by(3) {
        plain.insert(i);
    }
    let atomic = AtomicBitSet::<AtomicU64, usize>::from_bits(AtomicU64::new(plain.into_bits()));

    assert_eq!(atomic.len(), plain.len(), "len mismatch");
    assert_eq!(
        atomic.contains(&probe),
        plain.contains(&probe),
        "contains mismatch"
    );

    let mut g = c.benchmark_group("64bit_atomic");

    g.bench_function("atomic/len", |b| b.iter(|| black_box(&atomic).len()));
    g.bench_function("plain/len", |b| b.iter(|| black_box(&plain).len()));

    g.bench_function("atomic/is_empty", |b| {
        b.iter(|| black_box(&atomic).is_empty())
    });
    g.bench_function("plain/is_empty", |b| {
        b.iter(|| black_box(&plain).is_empty())
    });

    g.bench_function("atomic/contains", |b| {
        b.iter(|| black_box(&atomic).contains(black_box(&probe)))
    });
    g.bench_function("plain/contains", |b| {
        b.iter(|| black_box(&plain).contains(black_box(&probe)))
    });

    g.bench_function("atomic/insert", |b| {
        b.iter(|| {
            let s = AtomicBitSet::<AtomicU64, usize>::new();
            s.insert(black_box(probe));
            black_box(&s);
        })
    });
    g.bench_function("plain/insert", |b| {
        b.iter(|| {
            let mut s = BitSet::<u64, usize>::new();
            s.insert(black_box(probe));
            black_box(&s);
        })
    });

    g.bench_function("atomic/remove", |b| {
        b.iter(|| {
            atomic.insert(black_box(probe));
            atomic.remove(black_box(probe));
        })
    });
    g.bench_function("plain/remove", |b| {
        b.iter(|| {
            let mut s = plain;
            s.remove(black_box(probe));
            black_box(&s);
        })
    });

    g.bench_function("atomic/iter", |b| {
        b.iter(|| black_box(&atomic).iter().count())
    });
    g.bench_function("plain/iter", |b| {
        b.iter(|| black_box(&plain).iter().count())
    });

    g.finish();
}

// ── array: AtomicBitSet<[AtomicU64; 4]> vs BitSet<[u64; 4]> ────

fn bench_atomic_256(c: &mut Criterion) {
    let probe: usize = 200;

    let mut plain_a = BitSet::<[u64; 4], usize>::new();
    for i in (0..256).step_by(3) {
        plain_a.insert(i);
    }
    let raw_a = plain_a.into_bits();
    let atomic_a = AtomicBitSet::<[AtomicU64; 4], usize>::from_bits(raw_a.map(AtomicU64::new));

    let mut plain_b = BitSet::<[u64; 4], usize>::new();
    for i in (0..256).step_by(5) {
        plain_b.insert(i);
    }
    let raw_b = plain_b.into_bits();
    let atomic_b = AtomicBitSet::<[AtomicU64; 4], usize>::from_bits(raw_b.map(AtomicU64::new));

    assert_eq!(atomic_a.len(), plain_a.len(), "len mismatch");
    assert_eq!(
        atomic_a.contains(&probe),
        plain_a.contains(&probe),
        "contains mismatch"
    );

    let mut g = c.benchmark_group("256bit_atomic");

    g.bench_function("atomic/len", |b| b.iter(|| black_box(&atomic_a).len()));
    g.bench_function("plain/len", |b| b.iter(|| black_box(&plain_a).len()));

    g.bench_function("atomic/is_empty", |b| {
        b.iter(|| black_box(&atomic_a).is_empty())
    });
    g.bench_function("plain/is_empty", |b| {
        b.iter(|| black_box(&plain_a).is_empty())
    });

    g.bench_function("atomic/contains", |b| {
        b.iter(|| black_box(&atomic_a).contains(black_box(&probe)))
    });
    g.bench_function("plain/contains", |b| {
        b.iter(|| black_box(&plain_a).contains(black_box(&probe)))
    });

    g.bench_function("atomic/is_subset", |b| {
        b.iter(|| black_box(&atomic_b).is_subset(black_box(&atomic_a)))
    });
    g.bench_function("plain/is_subset", |b| {
        b.iter(|| black_box(&plain_b).is_subset(black_box(&plain_a)))
    });

    g.bench_function("atomic/insert", |b| {
        b.iter(|| {
            let s = AtomicBitSet::<[AtomicU64; 4], usize>::new();
            s.insert(black_box(probe));
            black_box(&s);
        })
    });
    g.bench_function("plain/insert", |b| {
        b.iter(|| {
            let mut s = BitSet::<[u64; 4], usize>::new();
            s.insert(black_box(probe));
            black_box(&s);
        })
    });

    g.bench_function("atomic/iter", |b| {
        b.iter(|| black_box(&atomic_a).iter().count())
    });
    g.bench_function("plain/iter", |b| {
        b.iter(|| black_box(&plain_a).iter().count())
    });

    g.finish();
}

// ── boxed: AtomicBoxedBitSet<AtomicU64> vs BoxedBitSet<u64> ─────

fn bench_atomic_boxed(c: &mut Criterion) {
    const BITS: usize = 65536;
    let probe: usize = 50000;

    let mut plain = BoxedBitSet::<u64, usize>::with_capacity(BITS);
    for i in (0..BITS).step_by(3) {
        plain.insert(i);
    }
    let atomic_store: Vec<AtomicU64> = plain
        .as_raw_slice()
        .iter()
        .map(|&v| AtomicU64::new(v))
        .collect();
    let atomic =
        AtomicBoxedBitSet::<AtomicU64, usize>::from_boxed_slice(atomic_store.into_boxed_slice());

    assert_eq!(atomic.len(), plain.len(), "len mismatch");
    assert_eq!(
        atomic.contains(&probe),
        plain.contains(&probe),
        "contains mismatch"
    );

    let mut g = c.benchmark_group("65536bit_atomic_boxed");

    g.bench_function("atomic/len", |b| b.iter(|| black_box(&*atomic).len()));
    g.bench_function("plain/len", |b| b.iter(|| black_box(&*plain).len()));

    g.bench_function("atomic/is_empty", |b| {
        b.iter(|| black_box(&*atomic).is_empty())
    });
    g.bench_function("plain/is_empty", |b| {
        b.iter(|| black_box(&*plain).is_empty())
    });

    g.bench_function("atomic/contains", |b| {
        b.iter(|| black_box(&*atomic).contains(black_box(&probe)))
    });
    g.bench_function("plain/contains", |b| {
        b.iter(|| black_box(&*plain).contains(black_box(&probe)))
    });

    g.bench_function("atomic/insert", |b| {
        b.iter(|| {
            let s = AtomicBoxedBitSet::<AtomicU64, usize>::with_capacity(BITS);
            s.insert(black_box(probe));
            black_box(&s);
        })
    });
    g.bench_function("plain/insert", |b| {
        b.iter(|| {
            let mut s = BoxedBitSet::<u64, usize>::with_capacity(BITS);
            s.insert(black_box(probe));
            black_box(&s);
        })
    });

    g.bench_function("atomic/iter", |b| {
        b.iter(|| black_box(&*atomic).iter().count())
    });
    g.bench_function("plain/iter", |b| {
        b.iter(|| black_box(&*plain).iter().count())
    });

    g.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    targets =
        bench_atomic_64,
        bench_atomic_256,
        bench_atomic_boxed,
}
criterion_main!(benches);
