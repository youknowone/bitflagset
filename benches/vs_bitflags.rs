use std::hint::black_box;
use std::time::Duration;

use bitflagset::BitSet;
use criterion::{Criterion, criterion_group, criterion_main};

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct Flags: u64 {
        const BIT0 = 1u64 << 0;
    }
}

#[inline]
fn mask_of(idx: usize) -> Flags {
    Flags::from_bits_retain(1u64 << idx)
}

#[inline]
fn iter_ones_count(mut bits: u64) -> usize {
    let mut count = 0usize;
    while bits != 0 {
        bits &= bits - 1;
        count += 1;
    }
    count
}

fn bench_64_vs_bitflags(c: &mut Criterion) {
    let bits_a: Vec<usize> = (0..64).step_by(3).collect();
    let bits_b: Vec<usize> = (0..64).step_by(5).collect();
    let probe: usize = 42;

    let mut ours_a = BitSet::<u64, usize>::new();
    let mut ours_b = BitSet::<u64, usize>::new();
    let mut bf_a = Flags::empty();
    let mut bf_b = Flags::empty();

    for &i in &bits_a {
        ours_a.insert(i);
        bf_a.insert(mask_of(i));
    }
    for &i in &bits_b {
        ours_b.insert(i);
        bf_b.insert(mask_of(i));
    }

    // sanity checks: both representations contain same set bits
    assert_eq!(
        ours_a.len() as u32,
        bf_a.bits().count_ones(),
        "len mismatch"
    );
    assert_eq!(
        ours_a.contains(&probe),
        bf_a.contains(mask_of(probe)),
        "contains mismatch"
    );
    assert_eq!(ours_a.is_empty(), bf_a.is_empty(), "is_empty mismatch");

    let mut g = c.benchmark_group("64bit_vs_bitflags");

    g.bench_function("ours/len", |b| b.iter(|| black_box(&ours_a).len()));
    g.bench_function("bitflags/len", |b| {
        b.iter(|| black_box(&bf_a).bits().count_ones())
    });

    g.bench_function("ours/is_empty", |b| {
        b.iter(|| black_box(&ours_a).is_empty())
    });
    g.bench_function("bitflags/is_empty", |b| {
        b.iter(|| black_box(&bf_a).is_empty())
    });

    g.bench_function("ours/contains", |b| {
        b.iter(|| black_box(&ours_a).contains(black_box(&probe)))
    });
    g.bench_function("bitflags/contains", |b| {
        b.iter(|| black_box(&bf_a).contains(black_box(mask_of(probe))))
    });

    g.bench_function("ours/is_subset", |b| {
        b.iter(|| black_box(&ours_b).is_subset(black_box(&ours_a)))
    });
    g.bench_function("bitflags/is_subset", |b| {
        b.iter(|| black_box(&bf_a).contains(black_box(bf_b)))
    });

    g.bench_function("ours/bitor", |b| {
        b.iter(|| black_box(ours_a) | black_box(ours_b))
    });
    g.bench_function("bitflags/bitor", |b| {
        b.iter(|| black_box(bf_a) | black_box(bf_b))
    });

    g.bench_function("ours/bitand", |b| {
        b.iter(|| black_box(ours_a) & black_box(ours_b))
    });
    g.bench_function("bitflags/bitand", |b| {
        b.iter(|| black_box(bf_a) & black_box(bf_b))
    });

    g.bench_function("ours/bitxor", |b| {
        b.iter(|| black_box(ours_a) ^ black_box(ours_b))
    });
    g.bench_function("bitflags/bitxor", |b| {
        b.iter(|| black_box(bf_a) ^ black_box(bf_b))
    });

    g.bench_function("ours/not", |b| b.iter(|| !black_box(ours_a)));
    g.bench_function("bitflags/not", |b| b.iter(|| !black_box(bf_a)));

    g.bench_function("ours/sub", |b| {
        b.iter(|| black_box(ours_a) - black_box(ours_b))
    });
    g.bench_function("bitflags/sub", |b| {
        b.iter(|| black_box(bf_a) - black_box(bf_b))
    });

    g.bench_function("ours/insert", |b| {
        b.iter(|| {
            let mut s = ours_a;
            s.insert(black_box(probe));
            black_box(s);
        })
    });
    g.bench_function("bitflags/insert", |b| {
        b.iter(|| {
            let mut s = bf_a;
            s.insert(black_box(mask_of(probe)));
            black_box(s);
        })
    });

    g.bench_function("ours/remove", |b| {
        b.iter(|| {
            let mut s = ours_a;
            s.remove(black_box(probe));
            black_box(s);
        })
    });
    g.bench_function("bitflags/remove", |b| {
        b.iter(|| {
            let mut s = bf_a;
            s.remove(black_box(mask_of(probe)));
            black_box(s);
        })
    });

    g.bench_function("ours/iter", |b| {
        b.iter(|| black_box(&ours_a).iter().count())
    });
    g.bench_function("bitflags/iter_bits", |b| {
        b.iter(|| iter_ones_count(black_box(&bf_a).bits()))
    });

    g.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    targets = bench_64_vs_bitflags,
}
criterion_main!(benches);
