# bitflagset

Type-safe bitsets with `Set`-like ergonomics. Operations are direct primitive bit operations over words. Optional bitvec interop via the `bitvec` feature.

## Design philosophy

**Element-centric, not mask-centric.** Every type exposes a `HashSet`/`BTreeSet`-style interface — `contains(&V)`, `insert(V) -> bool`, `remove(V) -> bool`, `is_subset`, ... — while the underlying storage is a compact bit vector. The element type `V` is a generic parameter: an enum, a `usize`, or a named position constant. You work with domain values, never with raw bitmasks.

**Deref-based method sharing.** `BitSet`, `BoxedBitSet`, and array-backed `BitSet<[T; N], V>` all `Deref` to a shared unsized `BitSlice<T, V>` — a `#[repr(transparent)]` wrapper around `[T]`. Atomic types similarly share `AtomicBitSlice<A, V>`. Common methods are defined once on the slice type; owned types add storage-specific operations on top.

**Word-level primitive operations.** `BitSlice` operates on the raw `[T]` slice directly using `count_ones()`, bit masking, and word-level boolean operators.

**Const-friendly primitives.** For single-primitive `BitSet<u64, V>`, all query methods (`len`, `is_empty`, `is_subset`, ...) and constructors (`from_bits`, `from_index`, `from_indices`) are `const fn`. Build complex bitsets at compile time with zero runtime cost.

**Full bit-operator support.** Non-atomic owned types (`BitSet`, `BoxedBitSet`) implement `BitOr`, `BitAnd`, `BitXor`, `Not`, `Sub` and their `Assign` variants. Atomic types (`AtomicBitSet`, `AtomicBoxedBitSet`) expose equivalent set operations via named methods (`union`, `difference`, etc.) since atomics are inherently `&self`-based.

## Performance

<!-- BENCH_TABLES:BEGIN -->
All numbers below are Criterion medians from `cargo bench --bench compare`, run on Apple M-series (AArch64), collected on **2026-09-28**. This is a shared-machine measurement.  
Load at bench time: before `13:02  up 4 days, 13:22, 52 users, load averages: 88.03 91.00 78.54`; after `13:12  up 4 days, 13:32, 52 users, load averages: 52.69 68.25 72.17`.
Each competitor cell is that library's median and how many times bitflagset is faster (`other / bitflagset`). The bitflagset column and every competitor in that row come from the **same** Criterion group, so the printed ratio is `competitor / bitflagset` on the printed times (rounded to 0.1). `—` means that library has no equivalent in the bench.  
`iter`, `union`, `intersection`, and `difference` count iterator items. `insert`, `remove`, `set`, `set_false`, `clear`, `union_with` / `or`, `intersect_with` / `and`, and `difference_with` clone the destination in `iter_batched_ref` setup and time only the operation. Those in-place ops borrow the other set (`union_from`, `intersect_from`, `difference_from`), matching bit-set `*_with` and bit-vec `or` / `and` / `difference`. Fixed-size rows use `BatchSize::SmallInput`; the 65536-bit rows use `BatchSize::LargeInput`. `len` / `count` is `len` / `count` / `count_ones`. `clear` is `clear` / `make_empty` / `fill(false)`. `contains` / `get` is `contains` / `get`. bitvec `bitor` / `bitand` / `bitxor` / `not` are by-value operators that build a new set. Atomic `insert` builds a fresh set and sets one bit (`set_aliased` on bitvec).

### Non-atomic

`BitSet<[u64; N]>` / `BoxedBitSet<u64>` against bitvec `BitArray` / `BitVec<u64>`, `bit_set::BitSet` (default `u32` blocks, `with_capacity`), and `bit_vec::BitVec` (default `u32` blocks, `from_elem`). One group per size measures bitflagset and all three libraries.

**256-bit** (`[u64; 4]`):

| Operation | bitflagset | bitvec | bit-set | bit-vec |
|-----------|------------|--------|---------|---------|
| `insert` | 0.86 ns | — | 2.00 ns (2.3x) | — |
| `remove` | 0.69 ns | — | 1.59 ns (2.3x) | — |
| `contains / get` | 1.28 ns | 1.33 ns (1.0x) | 1.27 ns (1.0x) | 1.39 ns (1.1x) |
| `len / count` | 0.96 ns | 5.41 ns (5.6x) | 4.37 ns (4.6x) | 4.06 ns (4.2x) |
| `is_subset` | 1.05 ns | — | 1.59 ns (1.5x) | — |
| `iter` | 0.95 ns | 5.36 ns (5.6x) | 4.31 ns (4.5x) | — |
| `clear` | 2.46 ns | — | 3.49 ns (1.4x) | 3.35 ns (1.4x) |
| `set` | 0.89 ns | — | — | 2.32 ns (2.6x) |
| `set_false` | 0.62 ns | — | — | 2.23 ns (3.6x) |
| `union` | 2.59 ns | — | 10.71 ns (4.1x) | — |
| `intersection` | 2.78 ns | — | 11.01 ns (4.0x) | — |
| `difference` | 2.92 ns | — | 9.27 ns (3.2x) | — |
| `union_with / or` | 2.46 ns | — | 9.67 ns (3.9x) | 7.97 ns (3.2x) |
| `intersect_with / and` | 2.24 ns | — | 9.67 ns (4.3x) | 7.64 ns (3.4x) |
| `difference_with` | 2.23 ns | — | 8.79 ns (3.9x) | 7.84 ns (3.5x) |
| `bitor` | 2.71 ns | 54.99 ns (20.3x) | — | — |
| `bitand` | 2.61 ns | 60.11 ns (23.0x) | — | — |
| `bitxor` | 2.84 ns | 56.39 ns (19.9x) | — | — |
| `not` | 1.90 ns | 2.93 ns (1.5x) | — | — |

**1024-bit** (`[u64; 16]`):

| Operation | bitflagset | bitvec | bit-set | bit-vec |
|-----------|------------|--------|---------|---------|
| `insert` | 3.09 ns | — | 3.02 ns (1.0x) | — |
| `remove` | 3.28 ns | — | 3.17 ns (1.0x) | — |
| `contains / get` | 1.19 ns | 1.21 ns (1.0x) | 1.35 ns (1.1x) | 1.27 ns (1.1x) |
| `len / count` | 3.23 ns | 18.35 ns (5.7x) | 4.80 ns (1.5x) | 5.18 ns (1.6x) |
| `is_subset` | 0.96 ns | — | 1.53 ns (1.6x) | — |
| `iter` | 3.17 ns | 17.51 ns (5.5x) | 9.98 ns (3.1x) | — |
| `clear` | 2.91 ns | — | 3.53 ns (1.2x) | 3.28 ns (1.1x) |
| `set` | 3.57 ns | — | — | 3.51 ns (1.0x) |
| `set_false` | 2.30 ns | — | — | 3.84 ns (1.7x) |
| `union` | 12.21 ns | — | 32.74 ns (2.7x) | — |
| `intersection` | 12.33 ns | — | 33.73 ns (2.7x) | — |
| `difference` | 8.87 ns | — | 34.34 ns (3.9x) | — |
| `union_with / or` | 4.70 ns | — | 25.00 ns (5.3x) | 7.45 ns (1.6x) |
| `intersect_with / and` | 4.65 ns | — | 26.73 ns (5.7x) | 6.99 ns (1.5x) |
| `difference_with` | 4.51 ns | — | 24.64 ns (5.5x) | 7.55 ns (1.7x) |
| `bitor` | 10.35 ns | 230.18 ns (22.2x) | — | — |
| `bitand` | 9.69 ns | 219.14 ns (22.6x) | — | — |
| `bitxor` | 9.95 ns | 224.74 ns (22.6x) | — | — |
| `not` | 6.34 ns | 10.79 ns (1.7x) | — | — |

**65536-bit** (`BoxedBitSet`):

| Operation | bitflagset | bitvec | bit-set | bit-vec |
|-----------|------------|--------|---------|---------|
| `insert` | 3.85 ns | — | 5.60 ns (1.5x) | — |
| `remove` | 2.22 ns | — | 3.03 ns (1.4x) | — |
| `contains / get` | 1.36 ns | 1.48 ns (1.1x) | 1.31 ns (1.0x) | 1.29 ns (0.9x) |
| `len / count` | 170.97 ns | 1.04 us (6.1x) | 224.59 ns (1.3x) | 228.79 ns (1.3x) |
| `is_subset` | 1.33 ns | — | 1.55 ns (1.2x) | — |
| `iter` | 170.51 ns | 967.78 ns (5.7x) | 228.65 ns (1.3x) | — |
| `clear` | 174.37 ns | — | 176.29 ns (1.0x) | 156.96 ns (0.9x) |
| `set` | 3.03 ns | — | — | 5.18 ns (1.7x) |
| `set_false` | 2.24 ns | — | — | 4.98 ns (2.2x) |
| `union` | 736.97 ns | — | 2.02 us (2.7x) | — |
| `intersection` | 758.68 ns | — | 1.91 us (2.5x) | — |
| `difference` | 769.80 ns | — | 1.90 us (2.5x) | — |
| `union_with / or` | 229.65 ns | — | 1.37 us (6.0x) | 245.82 ns (1.1x) |
| `intersect_with / and` | 234.02 ns | — | 1.33 us (5.7x) | 226.50 ns (1.0x) |
| `difference_with` | 265.78 ns | — | 1.36 us (5.1x) | 260.80 ns (1.0x) |
| `bitor` | 540.07 ns | 17.47 us (32.3x) | — | — |
| `bitand` | 517.18 ns | 15.86 us (30.7x) | — | — |
| `bitxor` | 541.86 ns | 18.30 us (33.8x) | — | — |
| `not` | 288.68 ns | 1.05 us (3.6x) | — | — |

`first` / `last` are omitted: `bit-set` and `bit-vec` have neither. bit-set has no owned `|` / `&` / `-`; those producing operators are the bitvec `bitor` / `bitand` / `bitxor` / `not` rows. Iterator `union` / `intersection` / `difference` allocate nothing and count yielded indices. `union_with` / `or`, `intersect_with` / `and`, and `difference_with` are in place (`union_from`, `intersect_from`, `difference_from` versus bit-set `*_with` and bit-vec `or` / `and` / `difference`). `bit-vec` `insert` / `remove` shift the vector, so membership writes are `set` / `set_false`. `bit-vec` has no `is_subset` and no set-index iterator.

Rows that are still slower, and why:
- `256-bit contains / get vs bit-set`: bitflagset 1.28 ns, competitor 1.27 ns. Both read one word. The final paired sample is 1.28 ns vs 1.27 ns.
- `65536-bit contains / get vs bit-set`: bitflagset 1.36 ns, competitor 1.31 ns. Both read one word. Five interleaved runs against bit-vec, the same one-word load, overlapped; this bit-set sample is 1.36 ns vs 1.31 ns.
- `65536-bit contains / get vs bit-vec`: bitflagset 1.36 ns, competitor 1.29 ns. Both read one word. Five interleaved medians overlap (ours 1.11–1.73 ns, bit-vec 1.19–1.97 ns).
- `1024-bit insert`: bitflagset 3.09 ns, competitor 3.02 ns. Both set one bit in a pre-sized buffer. The final paired sample is 3.09 ns vs 3.02 ns.
- `1024-bit remove`: bitflagset 3.28 ns, competitor 3.17 ns. Both clear one bit in a pre-sized buffer. The final paired sample is 3.28 ns vs 3.17 ns.
- `1024-bit set`: bitflagset 3.57 ns, competitor 3.51 ns. Both set one bit in a pre-sized buffer. The final paired sample is 3.57 ns vs 3.51 ns.
- `65536-bit intersect_with / and`: bitflagset 234.02 ns, competitor 226.50 ns. Both use NEON `and.16b`, 64 bytes per iteration. bit-vec also folds a changed-bit flag. Five interleaved medians overlap.
- `65536-bit difference_with`: bitflagset 265.78 ns, competitor 260.80 ns. Both use NEON `bic.16b`, 64 bytes per iteration. bit-vec also builds a changed-bit mask. Five interleaved medians overlap (ours 257–441 ns, bit-vec 256–301 ns).
- `65536-bit clear vs bit-vec`: bitflagset 174.37 ns, competitor 156.96 ns. Both zero 8 KiB with `memset`. Five interleaved medians overlap (ours 144–160 ns, bit-vec 142–183 ns).

### Atomic

`AtomicBitSet<[AtomicU64; N]>` / `AtomicBoxedBitSet` against bitvec `BitArray<AtomicU64>` / `BitVec<AtomicU64>`. bit-set and bit-vec have no atomic storage.

**256-bit** (`[AtomicU64; 4]`):

| Operation | bitflagset | bitvec | bit-set | bit-vec |
|-----------|------------|--------|---------|---------|
| `len` | 1.55 ns | 5.61 ns (3.6x) | — | — |
| `is_empty` | 0.60 ns | 5.73 ns (9.6x) | — | — |
| `contains` | 1.21 ns | 1.11 ns (0.9x) | — | — |
| `insert` | 1.81 ns | 2.45 ns (1.4x) | — | — |
| `iter` | 2.02 ns | 7.89 ns (3.9x) | — | — |

**1024-bit** (`[AtomicU64; 16]`):

| Operation | bitflagset | bitvec | bit-set | bit-vec |
|-----------|------------|--------|---------|---------|
| `len` | 5.32 ns | 22.58 ns (4.2x) | — | — |
| `is_empty` | 0.68 ns | 19.80 ns (29.1x) | — | — |
| `contains` | 1.18 ns | 1.03 ns (0.9x) | — | — |
| `insert` | 3.13 ns | 3.81 ns (1.2x) | — | — |
| `iter` | 5.77 ns | 18.48 ns (3.2x) | — | — |

**65536-bit** (`AtomicBoxedBitSet`):

| Operation | bitflagset | bitvec | bit-set | bit-vec |
|-----------|------------|--------|---------|---------|
| `len` | 538.30 ns | 969.05 ns (1.8x) | — | — |
| `is_empty` | 0.80 ns | 961.62 ns (1202.0x) | — | — |
| `contains` | 1.14 ns | 1.14 ns (1.0x) | — | — |
| `insert` | 179.26 ns | — | — | — |
| `iter` | 483.44 ns | 962.34 ns (2.0x) | — | — |

`is_empty` returns on the first non-zero word. Atomic `contains` loads the word with `Relaxed` ordering. The 65536-bit atomic `insert` column is `—` for bitvec: that bench does not include an aliased set on `BitVec<AtomicU64>`.

Rows that are still slower, and why:
- `256-bit atomic contains`: bitflagset 1.21 ns, competitor 1.11 ns. Both are one `Relaxed` load plus an in-range check. Interleaved samples sit on top of each other around 1–3 ns.
- `1024-bit atomic contains`: bitflagset 1.18 ns, competitor 1.03 ns. Same `Relaxed` load and in-range check as the 256-bit row. Five interleaved medians overlap (ours 1.00–2.58 ns, bitvec 0.98–2.59 ns). The ordering is not `SeqCst`.
<!-- BENCH_TABLES:END -->

## Types

| Type | Storage | Thread-safe | Deref target |
|------|---------|-------------|--------------|
| `BitSet<A, V>` | Single primitive | No | `BitSlice<A, V>` |
| `BitSet<[T; N], V>` | Fixed-size array | No | `BitSlice<T, V>` |
| `BoxedBitSet<T, V>` | Heap `Box<[T]>` | No | `BitSlice<T, V>` |
| `AtomicBitSet<A, V>` | Atomic primitive | Yes | *(direct methods)* |
| `AtomicBitSet<[A; N], V>` | Atomic array | Yes | `AtomicBitSlice<A, V>` |
| `AtomicBoxedBitSet<A, V>` | Heap `Box<[A]>` | Yes | `AtomicBitSlice<A, V>` |

Type aliases `ArrayBitSet<A, V, N>` and `AtomicArrayBitSet<A, V, N>` are provided for convenience.

### Set interface

All types provide the standard collection methods:

```rust
use bitflagset::BitSet;

let mut a = BitSet::<u64, usize>::new();
a.insert(3);
a.insert(7);
a.insert(42);

assert!(a.contains(&7));
assert_eq!(a.len(), 3);

a.remove(7);
assert!(!a.contains(&7));

// Set algebra
let b = BitSet::<u64, usize>::from_element(3);
assert!(b.is_subset(&a));
assert!(a.is_superset(&b));
```

### Bounds behavior

For index-based element types (for example `usize`), runtime-index operations
(`contains`, `insert`, `remove`, `set`, `toggle`) include debug assertions for
out-of-range indices.

- In debug builds, out-of-range indices trigger assertion failures.
- In release builds, out-of-range indices are ignored (`contains`/`insert`/`remove` return `false`; `set`/`toggle` are no-ops).

This surfaces mistakes during development while keeping release builds branch-light.

### Const construction

Primitive-backed `BitSet` supports const construction and queries:

```rust
use bitflagset::BitSet;

const FLAGS: BitSet<u64, usize> = BitSet::<u64, usize>::from_indices(&[3, 7, 42]);
const SINGLE: BitSet<u64, usize> = BitSet::<u64, usize>::from_index(5);
const RAW: BitSet<u64, usize> = BitSet::<u64, usize>::from_bits(0b1010);

// Const queries
const _: () = assert!(FLAGS.contains(&3));
const _: () = assert!(FLAGS.len() == 3);
const _: () = assert!(SINGLE.is_disjoint(&FLAGS));
```

### Bit operators

```rust
use bitflagset::BitSet;

let a = [1u8, 4, 9].into_iter().collect::<BitSet<u64, u8>>();
let b = [4u8, 9, 15].into_iter().collect::<BitSet<u64, u8>>();

let union        = a | b;   // BitOr
let intersection = a & b;   // BitAnd
let sym_diff     = a ^ b;   // BitXor
let difference   = a - b;   // Sub
let complement   = !a;      // Not
```

All operators also have `Assign` variants (`|=`, `&=`, `^=`, `-=`), including borrowed right-hand sides (`|= &other`). `union_from`, `intersect_from`, `difference_from`, and `symmetric_difference_from` are the named borrowed in-place forms.

### Atomic bitsets

`AtomicBitSet` provides the same interface but all operations take `&self`. Mutations use `AcqRel` ordering; read-only methods (`len`, `contains`, `iter`, ...) use `Relaxed`.

```rust
use bitflagset::AtomicBitSet;
use std::sync::atomic::AtomicU64;

let flags = AtomicBitSet::<AtomicU64, usize>::new();
flags.insert(10);  // &self, not &mut self
assert!(flags.contains(&10));
```

### Array-backed bitsets

For bit widths beyond a single primitive:

```rust
use bitflagset::BitSet;

let mut bs = BitSet::<[u64; 4], usize>::new(); // 256 bits
bs.insert(0);
bs.insert(127);
bs.insert(200);
let items: Vec<usize> = bs.iter().collect();
assert_eq!(items, vec![0, 127, 200]);
```

### `bitflagset!` macro

Generates a named bitset type with element-centric `Set` API.

**Enum form:** wraps a `#[repr(u8)]` enum. Element type is the enum itself.

```rust
use bitflagset::{bitflag, bitflagset};

bitflag! {
    #[derive(Debug)]
    #[repr(u8)]
    enum Color {
        Red = 0,
        Green = 1,
        Blue = 2,
    }
}

bitflagset!(pub struct ColorSet(u8) : Color);

let mut set = ColorSet::from_slice(&[Color::Red, Color::Blue]);
assert!(set.contains(&Color::Red));
set.remove(Color::Green);

// Zero-cost conversion to/from BitSet
let bs: bitflagset::BitSet<u8, Color> = set.into();
let back: ColorSet = bs.into();
```

**Position form:** defines named constants as bit positions (not masks). Element type is `u8`.

```rust
use bitflagset::bitflagset;

bitflagset! {
    pub struct Perms(u8) {
        const READ = 0;
        const WRITE = 1;
        const EXEC = 2;
    }
}

let mut p = Perms::from_element(Perms::READ);
p.insert(Perms::WRITE);
assert!(p.contains(&Perms::READ));
assert_eq!(p.len(), 2);

// Composite constants via from_slice
impl Perms {
    const RW: Self = Self::from_slice(&[Self::READ, Self::WRITE]);
}
assert_eq!(p, Perms::RW);

// Same operators as enum form
let diff = Perms::all() - Perms::RW;
assert_eq!(diff, Perms::from_element(Perms::EXEC));
```

Both forms share the same `Set`-like interface: `contains(&V)`, `insert(V) -> bool`, `remove(V) -> bool`, `is_subset`, `is_superset`, `is_disjoint`, plus full bit operators (`|`, `&`, `^`, `-`, `!`).

Generated `bitflagset!` and `atomic_bitflagset!` structs are `#[repr(transparent)]` over their storage (`u64`, `AtomicU64`, ...), and `AtomicBitSet<A, V>` has the same layout as `A`. `#[cfg]` (and doc comments) on a position-form constant apply everywhere that constant is named, so a disabled flag is omitted from `all()`, names, and `Debug`. `atomic_bitflagset!(struct AtomicX(AtomicU64) on X)` links either an enum-form or a position-form `X` without repeating its constants.

### bitvec interop (optional)

Enable the `bitvec` feature to get zero-cost conversions:

```rust
use bitflagset::BitSet;

let bs = BitSet::<u64, usize>::from_element(5);
let raw: &bitvec::slice::BitSlice<u64, bitvec::order::Lsb0> = bs.as_bitvec_slice();
assert!(raw[5]);
```

## vs bitflags

| | bitflags | bitflagset |
|---|---|---|
| Mental model | mask-centric — constants are pre-shifted masks (`0b01`), API is mask-vs-mask (`contains(Self)`) | element-centric — constants are positions (`0, 1, 2`), API is set-vs-element (`contains(&V)`, `insert(V) -> bool`) |
| Atomic flags | Not provided; use `Mutex` or manual `AtomicU*` | `atomic_bitflagset!` provides lock-free `insert`/`remove`/`toggle` |

## License

MIT
