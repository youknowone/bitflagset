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
Load at bench time: before `16:18  up 4 days, 16:38, 52 users, load averages: 35.95 33.19 42.89`; after `16:28  up 4 days, 16:48, 52 users, load averages: 42.79 36.24 40.45`.
Each competitor is two columns: its median, then `×` = `their time / bitflagset` on the **printed** times (one decimal, or an integer at 100 or above, e.g. `2.3×`, `1202×`). The bitflagset time and every competitor in that row come from the same Criterion group. A ratio in italics is a row where bitflagset's raw median is slower. `—` means that library has no equivalent in the bench. A column pair that is `—` on every row is dropped, and a row with no competitor value is dropped.  
`iter`, `union`, `intersection`, and `difference` count iterator items. `insert`, `remove`, `set`, `set_false`, `clear`, `union_with` / `or`, `intersect_with` / `and`, and `difference_with` clone the destination in `iter_batched_ref` setup and time only the operation. Those in-place ops borrow the other set (`union_from`, `intersect_from`, `difference_from`), matching bit-set `*_with` and bit-vec `or` / `and` / `difference`. Fixed-size rows use `BatchSize::SmallInput`; the 65536-bit rows use `BatchSize::LargeInput`. `len` / `count` is `len` / `count` / `count_ones`. `clear` is `clear` / `make_empty` / `fill(false)`. `contains` / `get` is `contains` / `get`. bitvec `bitor` / `bitand` / `bitxor` / `not` are by-value operators that build a new set. Atomic `insert` builds a fresh set and sets one bit (`set_aliased` on bitvec). On the 256/1024/65536 tables, `(absent)` uses a clear bit and `(present)` uses a set bit.

### Non-atomic

`BitSet<[u64; N]>` / `BoxedBitSet<u64>` against bitvec `BitArray` / `BitVec<u64>`, `bit_set::BitSet` (default `u32` blocks, `with_capacity`), and `bit_vec::BitVec` (default `u32` blocks, `from_elem`). One group per size measures bitflagset and all three libraries.

**256-bit** (`[u64; 4]`):

| Operation              | bitflagset |   bitvec |     × |  bit-set |    × | bit-vec |    × |
| ---------------------- | ---------: | -------: | ----: | -------: | ---: | ------: | ---: |
| `insert`               |    0.59 ns |        — |     — |  1.93 ns | 3.3× |       — |    — |
| `remove (absent)`      |    0.58 ns |        — |     — |  1.39 ns | 2.4× |       — |    — |
| `remove (present)`     |    0.68 ns |        — |     — |  1.41 ns | 2.1× |       — |    — |
| `contains / get`       |    0.80 ns |  0.88 ns |  1.1× |  0.87 ns | 1.1× | 2.77 ns | 3.5× |
| `len / count`          |    0.62 ns |  2.27 ns |  3.7× |  1.86 ns | 3.0× | 1.87 ns | 3.0× |
| `is_subset`            |    0.47 ns |        — |     — |  1.33 ns | 2.8× |       — |    — |
| `iter`                 |    0.82 ns |  2.97 ns |  3.6× |  2.52 ns | 3.1× |       — |    — |
| `clear`                |    1.63 ns |        — |     — |  2.50 ns | 1.5× | 2.48 ns | 1.5× |
| `set`                  |    0.68 ns |        — |     — |        — |    — | 1.86 ns | 2.7× |
| `set_false (absent)`   |    0.50 ns |        — |     — |        — |    — | 1.70 ns | 3.4× |
| `set_false (present)`  |    0.70 ns |        — |     — |        — |    — | 2.13 ns | 3.0× |
| `union`                |    2.57 ns |        — |     — | 12.18 ns | 4.7× |       — |    — |
| `intersection`         |    2.16 ns |        — |     — |  6.97 ns | 3.2× |       — |    — |
| `difference`           |    1.82 ns |        — |     — |  8.96 ns | 4.9× |       — |    — |
| `union_with / or`      |    1.52 ns |        — |     — |  7.58 ns | 5.0× | 2.75 ns | 1.8× |
| `intersect_with / and` |    2.00 ns |        — |     — |  6.12 ns | 3.1× | 2.77 ns | 1.4× |
| `difference_with`      |    1.29 ns |        — |     — |  6.59 ns | 5.1× | 2.61 ns | 2.0× |
| `bitor`                |    1.45 ns | 22.09 ns | 15.2× |        — |    — |       — |    — |
| `bitand`               |    1.51 ns | 31.17 ns | 20.6× |        — |    — |       — |    — |
| `bitxor`               |    2.07 ns | 24.48 ns | 11.8× |        — |    — |       — |    — |
| `not`                  |    1.44 ns |  1.98 ns |  1.4× |        — |    — |       — |    — |

**1024-bit** (`[u64; 16]`):

| Operation              | bitflagset |    bitvec |      × |  bit-set |    × | bit-vec |      × |
| ---------------------- | ---------: | --------: | -----: | -------: | ---: | ------: | -----: |
| `insert`               |    1.98 ns |         — |      — |  3.48 ns | 1.8× |       — |      — |
| `remove (absent)`      |    1.89 ns |         — |      — |  2.30 ns | 1.2× |       — |      — |
| `remove (present)`     |    2.57 ns |         — |      — |  2.69 ns | 1.0× |       — |      — |
| `contains / get`       |    0.78 ns |   1.20 ns |   1.5× |  0.96 ns | 1.2× | 4.08 ns |   5.2× |
| `len / count`          |    2.11 ns |   2.71 ns |   1.3× |  3.53 ns | 1.7× | 3.60 ns |   1.7× |
| `is_subset`            |    0.63 ns |         — |      — |  1.13 ns | 1.8× |       — |      — |
| `iter`                 |    2.47 ns |   2.43 ns | *1.0×* |  5.87 ns | 2.4× |       — |      — |
| `clear`                |    2.52 ns |         — |      — |  2.93 ns | 1.2× | 2.77 ns |   1.1× |
| `set`                  |    2.40 ns |         — |      — |        — |    — | 3.49 ns |   1.5× |
| `set_false (absent)`   |    2.51 ns |         — |      — |        — |    — | 3.49 ns |   1.4× |
| `set_false (present)`  |    3.32 ns |         — |      — |        — |    — | 3.20 ns | *1.0×* |
| `union`                |    8.35 ns |         — |      — | 46.44 ns | 5.6× |       — |      — |
| `intersection`         |    9.29 ns |         — |      — | 29.97 ns | 3.2× |       — |      — |
| `difference`           |    6.79 ns |         — |      — | 30.98 ns | 4.6× |       — |      — |
| `union_with / or`      |    3.95 ns |         — |      — | 24.55 ns | 6.2× | 6.04 ns |   1.5× |
| `intersect_with / and` |    4.18 ns |         — |      — | 21.09 ns | 5.0× | 5.72 ns |   1.4× |
| `difference_with`      |    3.70 ns |         — |      — | 20.40 ns | 5.5× | 6.82 ns |   1.8× |
| `bitor`                |    8.05 ns | 107.74 ns |  13.4× |        — |    — |       — |      — |
| `bitand`               |    6.80 ns | 100.63 ns |  14.8× |        — |    — |       — |      — |
| `bitxor`               |    5.87 ns | 109.65 ns |  18.7× |        — |    — |       — |      — |
| `not`                  |    4.70 ns |   6.65 ns |   1.4× |        — |    — |       — |      — |

**65536-bit** (`BoxedBitSet`):

| Operation              | bitflagset |    bitvec |     × |   bit-set |      × |   bit-vec |      × |
| ---------------------- | ---------: | --------: | ----: | --------: | -----: | --------: | -----: |
| `insert`               |    3.29 ns |         — |     — |   5.02 ns |   1.5× |         — |      — |
| `remove (absent)`      |    1.94 ns |         — |     — |   1.84 ns | *0.9×* |         — |      — |
| `remove (present)`     |    3.31 ns |         — |     — |   3.36 ns |   1.0× |         — |      — |
| `contains / get`       |    1.08 ns |   1.64 ns |  1.5× |   1.42 ns |   1.3× |   4.01 ns |   3.7× |
| `len / count`          |  124.42 ns | 155.42 ns |  1.2× | 165.33 ns |   1.3× | 223.18 ns |   1.8× |
| `is_subset`            |    0.81 ns |         — |     — |   1.02 ns |   1.3× |         — |      — |
| `iter`                 |  143.30 ns | 181.04 ns |  1.3× | 155.00 ns |   1.1× |         — |      — |
| `clear`                |  114.95 ns |         — |     — | 117.59 ns |   1.0× | 119.26 ns |   1.0× |
| `set`                  |    3.38 ns |         — |     — |         — |      — |   3.76 ns |   1.1× |
| `set_false (absent)`   |    1.44 ns |         — |     — |         — |      — |   3.63 ns |   2.5× |
| `set_false (present)`  |    4.65 ns |         — |     — |         — |      — |   5.46 ns |   1.2× |
| `union`                |  807.01 ns |         — |     — |   2.61 µs |   3.2× |         — |      — |
| `intersection`         |  653.95 ns |         — |     — |   1.76 µs |   2.7× |         — |      — |
| `difference`           |  471.89 ns |         — |     — |   1.72 µs |   3.6× |         — |      — |
| `union_with / or`      |  165.23 ns |         — |     — |   1.10 µs |   6.7× | 180.54 ns |   1.1× |
| `intersect_with / and` |  170.35 ns |         — |     — |   1.13 µs |   6.6× | 164.51 ns | *1.0×* |
| `difference_with`      |  149.99 ns |         — |     — |   1.39 µs |   9.3× | 158.22 ns |   1.1× |
| `bitor`                |  318.73 ns |   8.71 µs | 27.3× |         — |      — |         — |      — |
| `bitand`               |  319.26 ns |   6.55 µs | 20.5× |         — |      — |         — |      — |
| `bitxor`               |  301.23 ns |   6.46 µs | 21.4× |         — |      — |         — |      — |
| `not`                  |  171.29 ns | 630.76 ns |  3.7× |         — |      — |         — |      — |

`first` / `last` are omitted: `bit-set` and `bit-vec` have neither. bit-set has no owned `|` / `&` / `-`; those producing operators are the bitvec `bitor` / `bitand` / `bitxor` / `not` rows. Iterator `union` / `intersection` / `difference` allocate nothing and count yielded indices. `union_with` / `or`, `intersect_with` / `and`, and `difference_with` are in place (`union_from`, `intersect_from`, `difference_from` versus bit-set `*_with` and bit-vec `or` / `and` / `difference`). `bit-vec` `insert` / `remove` shift the vector, so membership writes are `set` / `set_false`. `bit-vec` has no `is_subset` and no set-index iterator.

Slower rows:

| Row                                         |      Ours |    Theirs | Cause                             |
| ------------------------------------------- | --------: | --------: | --------------------------------- |
| 1024-bit `iter` vs bitvec                   |   2.47 ns |   2.43 ns | both walk set bits                |
| 1024-bit `set_false (present)` vs bit-vec   |   3.32 ns |   3.20 ns | clears one set bit                |
| 65536-bit `remove (absent)` vs bit-set      |   1.94 ns |   1.84 ns | clears one clear bit              |
| 65536-bit `intersect_with / and` vs bit-vec | 170.35 ns | 164.51 ns | NEON and, plus a changed-bit flag |

### Atomic

`AtomicBitSet<[AtomicU64; N]>` / `AtomicBoxedBitSet` against bitvec `BitArray<AtomicU64>` / `BitVec<AtomicU64>`. bit-set and bit-vec have no atomic storage, so those columns are omitted.

**256-bit** (`[AtomicU64; 4]`):

| Operation  | bitflagset |  bitvec |      × |
| ---------- | ---------: | ------: | -----: |
| `len`      |    0.92 ns | 2.30 ns |   2.5× |
| `is_empty` |    0.48 ns | 2.51 ns |   5.2× |
| `contains` |    0.89 ns | 0.73 ns | *0.8×* |
| `insert`   |    1.30 ns | 1.55 ns |   1.2× |
| `iter`     |    1.32 ns | 3.03 ns |   2.3× |

**1024-bit** (`[AtomicU64; 16]`):

| Operation  | bitflagset |  bitvec |     × |
| ---------- | ---------: | ------: | ----: |
| `len`      |    3.45 ns | 4.46 ns |  1.3× |
| `is_empty` |    0.48 ns | 4.91 ns | 10.2× |
| `contains` |    0.77 ns | 0.78 ns |  1.0× |
| `insert`   |    2.37 ns | 2.57 ns |  1.1× |
| `iter`     |    4.46 ns | 5.91 ns |  1.3× |

**65536-bit** (`AtomicBoxedBitSet`):

| Operation  | bitflagset |    bitvec |      × |
| ---------- | ---------: | --------: | -----: |
| `len`      |  410.02 ns | 320.48 ns | *0.8×* |
| `is_empty` |    0.68 ns | 326.20 ns |   480× |
| `contains` |    0.99 ns |   1.02 ns |   1.0× |
| `iter`     |  339.09 ns | 357.83 ns |   1.1× |

`is_empty` returns on the first non-zero word. Atomic `contains` loads the word with `Relaxed` ordering. The 65536-bit atomic `insert` row is omitted: that bench has no aliased set on `BitVec<AtomicU64>`, so bitvec has no value there.

Slower rows:

| Row                          |      Ours |    Theirs | Cause                                                                        |
| ---------------------------- | --------: | --------: | ---------------------------------------------------------------------------- |
| 256-bit `contains` vs bitvec |   0.89 ns |   0.73 ns | reloads &index; taken bounds branch; 256 interleaved median 1.14 (0.78-1.66) |
| 65536-bit `len` vs bitvec    | 410.02 ns | 320.48 ns | relaxed load plus popcount per word                                          |

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
