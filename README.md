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
All numbers below are Criterion medians from `cargo bench --bench compare`, collected on **2026-09-30**.
Each competitor is two columns: its median, then `×` = `their time / bitflagset` on the **printed** times (one decimal, or an integer at 100 or above, e.g. `2.3×`, `1202×`). The bitflagset time and every competitor in that row come from the same Criterion group. A ratio in italics is a row where bitflagset's raw median is slower. `—` means that library has no equivalent in the bench. A column pair that is `—` on every row is dropped, and a row with no competitor value is dropped.  
`iter`, `union`, `intersection`, and `difference` count iterator items. `insert`, `remove`, `set`, `set_false`, `clear`, `union_with` / `or`, `intersect_with` / `and`, and `difference_with` clone the destination in `iter_batched_ref` setup and time only the operation. Those in-place ops borrow the other set (`union_from`, `intersect_from`, `difference_from`), matching bit-set `*_with` and bit-vec `or` / `and` / `difference`. Fixed-size rows use `BatchSize::SmallInput`; the 65536-bit rows use `BatchSize::LargeInput`. `len` / `count` is `len` / `count` / `count_ones`. `clear` is `clear` / `make_empty` / `fill(false)`. `contains` / `get` is `contains` / `get`. bitvec `bitor` / `bitand` / `bitxor` / `not` are by-value operators that build a new set. Atomic `insert` builds a fresh set and sets one bit (`set_aliased` on bitvec). On the 256/1024/65536 tables, `(absent)` uses a clear bit and `(present)` uses a set bit.

### Non-atomic

`BitSet<[u64; N]>` / `BoxedBitSet<u64>` against bitvec `BitArray` / `BitVec<u64>`, `bit_set::BitSet` (default `u32` blocks, `with_capacity`), and `bit_vec::BitVec` (default `u32` blocks, `from_elem`). One group per size measures bitflagset and all three libraries.

**256-bit** (`[u64; 4]`):

| Operation              | bitflagset |   bitvec |     × |  bit-set |    × | bit-vec |    × |
| ---------------------- | ---------: | -------: | ----: | -------: | ---: | ------: | ---: |
| `insert`               |    0.58 ns |        — |     — |  1.67 ns | 2.9× |       — |    — |
| `remove (absent)`      |    0.63 ns |        — |     — |  1.28 ns | 2.0× |       — |    — |
| `remove (present)`     |    0.83 ns |        — |     — |  1.52 ns | 1.8× |       — |    — |
| `contains / get`       |    0.82 ns |  1.09 ns |  1.3× |  1.17 ns | 1.4× | 1.09 ns | 1.3× |
| `index`                |    0.82 ns |  1.09 ns |  1.3× |        — |    — |       — |    — |
| `len / count`          |    2.09 ns |  8.45 ns |  4.0× |  5.01 ns | 2.4× | 5.01 ns | 2.4× |
| `is_subset`            |    0.82 ns |        — |     — |  1.69 ns | 2.1× |       — |    — |
| `iter`                 |    2.09 ns |  8.46 ns |  4.0× |  7.13 ns | 3.4× |       — |    — |
| `clear`                |    0.57 ns |        — |     — |  2.51 ns | 4.4× | 2.78 ns | 4.9× |
| `set`                  |    0.58 ns |        — |     — |        — |    — | 2.28 ns | 3.9× |
| `set_false (absent)`   |    0.63 ns |        — |     — |        — |    — | 1.99 ns | 3.2× |
| `set_false (present)`  |    0.83 ns |        — |     — |        — |    — | 1.97 ns | 2.4× |
| `union`                |    5.89 ns |        — |     — | 13.99 ns | 2.4× |       — |    — |
| `intersection`         |    5.18 ns |        — |     — | 13.75 ns | 2.7× |       — |    — |
| `difference`           |    6.07 ns |        — |     — | 13.77 ns | 2.3× |       — |    — |
| `union_with / or`      |    1.36 ns |        — |     — |  9.05 ns | 6.7× | 8.22 ns | 6.0× |
| `intersect_with / and` |    1.60 ns |        — |     — |  8.77 ns | 5.5× | 8.21 ns | 5.1× |
| `difference_with`      |    1.41 ns |        — |     — |  9.10 ns | 6.5× | 8.62 ns | 6.1× |
| `bitor`                |    2.85 ns | 51.58 ns | 18.1× |        — |    — |       — |    — |
| `bitand`               |    2.85 ns | 51.54 ns | 18.1× |        — |    — |       — |    — |
| `bitxor`               |    2.85 ns | 51.56 ns | 18.1× |        — |    — |       — |    — |
| `not`                  |    2.09 ns |  2.69 ns |  1.3× |        — |    — |       — |    — |

**1024-bit** (`[u64; 16]`):

| Operation              | bitflagset |    bitvec |     × |  bit-set |    × |  bit-vec |    × |
| ---------------------- | ---------: | --------: | ----: | -------: | ---: | -------: | ---: |
| `insert`               |    1.24 ns |         — |     — |  2.37 ns | 1.9× |        — |    — |
| `remove (absent)`      |    1.25 ns |         — |     — |  1.83 ns | 1.5× |        — |    — |
| `remove (present)`     |    1.62 ns |         — |     — |  2.28 ns | 1.4× |        — |    — |
| `contains / get`       |    0.82 ns |   1.09 ns |  1.3× |  1.16 ns | 1.4× |  1.09 ns | 1.3× |
| `index`                |    0.82 ns |   1.09 ns |  1.3× |        — |    — |        — |    — |
| `len / count`          |    7.59 ns |  32.47 ns |  4.3× | 18.25 ns | 2.4× | 18.25 ns | 2.4× |
| `is_subset`            |    0.82 ns |         — |     — |  1.70 ns | 2.1× |        — |    — |
| `iter`                 |    7.60 ns |  32.44 ns |  4.3× | 18.49 ns | 2.4× |        — |    — |
| `clear`                |    2.21 ns |         — |     — |  2.58 ns | 1.2× |  2.55 ns | 1.2× |
| `set`                  |    1.23 ns |         — |     — |        — |    — |  2.57 ns | 2.1× |
| `set_false (absent)`   |    1.25 ns |         — |     — |        — |    — |  2.58 ns | 2.1× |
| `set_false (present)`  |    1.54 ns |         — |     — |        — |    — |  2.57 ns | 1.7× |
| `union`                |    9.43 ns |         — |     — | 53.71 ns | 5.7× |        — |    — |
| `intersection`         |    9.16 ns |         — |     — | 53.69 ns | 5.9× |        — |    — |
| `difference`           |    9.30 ns |         — |     — | 53.72 ns | 5.8× |        — |    — |
| `union_with / or`      |    3.15 ns |         — |     — | 28.56 ns | 9.1× |  6.54 ns | 2.1× |
| `intersect_with / and` |    3.13 ns |         — |     — | 28.57 ns | 9.1× |  6.51 ns | 2.1× |
| `difference_with`      |    3.15 ns |         — |     — | 28.61 ns | 9.1× |  6.97 ns | 2.2× |
| `bitor`                |    9.81 ns | 201.14 ns | 20.5× |        — |    — |        — |    — |
| `bitand`               |    9.80 ns | 201.09 ns | 20.5× |        — |    — |        — |    — |
| `bitxor`               |    9.80 ns | 201.18 ns | 20.5× |        — |    — |        — |    — |
| `not`                  |    7.43 ns |   9.74 ns |  1.3× |        — |    — |        — |    — |

**65536-bit** (`BoxedBitSet`):

| Operation              | bitflagset |   bitvec |     × |  bit-set |    × |   bit-vec |    × |
| ---------------------- | ---------: | -------: | ----: | -------: | ---: | --------: | ---: |
| `insert`               |    1.34 ns |        — |     — |  2.21 ns | 1.6× |         — |    — |
| `remove (absent)`      |    1.64 ns |        — |     — |  1.80 ns | 1.1× |         — |    — |
| `remove (present)`     |    1.99 ns |        — |     — |  2.11 ns | 1.1× |         — |    — |
| `contains / get`       |    0.97 ns |  1.12 ns |  1.2× |  1.16 ns | 1.2× |   1.09 ns | 1.1× |
| `index`                |    0.97 ns |  1.12 ns |  1.2× |        — |    — |         — |    — |
| `len / count`          |  511.11 ns |  1.69 µs |  3.3× |  1.13 µs | 2.2× |   1.14 µs | 2.2× |
| `is_subset`            |    1.20 ns |        — |     — |  1.66 ns | 1.4× |         — |    — |
| `iter`                 |  511.19 ns |  1.69 µs |  3.3× |  1.13 µs | 2.2× |         — |    — |
| `clear`                |   77.43 ns |        — |     — | 77.54 ns | 1.0× |  77.64 ns | 1.0× |
| `set`                  |    1.32 ns |        — |     — |        — |    — |   2.76 ns | 2.1× |
| `set_false (absent)`   |    1.62 ns |        — |     — |        — |    — |   2.54 ns | 1.6× |
| `set_false (present)`  |    2.01 ns |        — |     — |        — |    — |   2.48 ns | 1.2× |
| `union`                |  570.26 ns |        — |     — |  3.36 µs | 5.9× |         — |    — |
| `intersection`         |  569.65 ns |        — |     — |  3.36 µs | 5.9× |         — |    — |
| `difference`           |  569.95 ns |        — |     — |  3.36 µs | 5.9× |         — |    — |
| `union_with / or`      |  178.61 ns |        — |     — |  1.70 µs | 9.5× | 190.21 ns | 1.1× |
| `intersect_with / and` |  178.78 ns |        — |     — |  1.70 µs | 9.5× | 190.46 ns | 1.1× |
| `difference_with`      |  178.76 ns |        — |     — |  1.70 µs | 9.5× | 216.20 ns | 1.2× |
| `bitor`                |  411.51 ns | 15.19 µs | 36.9× |        — |    — |         — |    — |
| `bitand`               |  408.83 ns | 14.63 µs | 35.8× |        — |    — |         — |    — |
| `bitxor`               |  411.12 ns | 15.20 µs | 37.0× |        — |    — |         — |    — |
| `not`                  |  243.82 ns |  1.02 µs |  4.2× |        — |    — |         — |    — |

`first` / `last` are omitted: `bit-set` and `bit-vec` have neither. bit-set has no owned `|` / `&` / `-`; those producing operators are the bitvec `bitor` / `bitand` / `bitxor` / `not` rows. Iterator `union` / `intersection` / `difference` allocate nothing and count yielded indices. `union_with` / `or`, `intersect_with` / `and`, and `difference_with` are in place (`union_from`, `intersect_from`, `difference_from` versus bit-set `*_with` and bit-vec `or` / `and` / `difference`). `bit-vec` `insert` / `remove` shift the vector, so membership writes are `set` / `set_false`. `bit-vec` has no `is_subset` and no set-index iterator.

Slower rows:

None.

### Atomic

`AtomicBitSet<[AtomicU64; N]>` / `AtomicBoxedBitSet` against bitvec `BitArray<AtomicU64>` / `BitVec<AtomicU64>`. bit-set and bit-vec have no atomic storage, so those columns are omitted.

**256-bit** (`[AtomicU64; 4]`):

| Operation  | bitflagset |  bitvec |      × |
| ---------- | ---------: | ------: | -----: |
| `len`      |    4.77 ns | 8.45 ns |   1.8× |
| `is_empty` |    0.82 ns | 8.45 ns |  10.3× |
| `contains` |    0.82 ns | 0.82 ns |   1.0× |
| `index`    |    0.82 ns | 0.82 ns | *1.0×* |
| `insert`   |    2.13 ns | 2.21 ns |   1.0× |
| `iter`     |    4.77 ns | 8.45 ns |   1.8× |

**1024-bit** (`[AtomicU64; 16]`):

| Operation  | bitflagset |   bitvec |      × |
| ---------- | ---------: | -------: | -----: |
| `len`      |   21.56 ns | 32.16 ns |   1.5× |
| `is_empty` |    0.55 ns | 32.18 ns |  58.5× |
| `contains` |    0.82 ns |  0.82 ns | *1.0×* |
| `index`    |    0.82 ns |  0.82 ns | *1.0×* |
| `insert`   |    2.59 ns |  3.26 ns |   1.3× |
| `iter`     |   21.56 ns | 32.17 ns |   1.5× |

**65536-bit** (`AtomicBoxedBitSet`):

| Operation  | bitflagset |    bitvec |     × |
| ---------- | ---------: | --------: | ----: |
| `len`      |    1.38 µs |   1.68 µs |  1.2× |
| `is_empty` |    0.82 ns |   1.69 µs | 2061× |
| `contains` |    1.09 ns |   1.10 ns |  1.0× |
| `index`    |    1.09 ns |   1.09 ns |  1.0× |
| `insert`   |  107.62 ns | 108.39 ns |  1.0× |
| `iter`     |    1.35 µs |   1.69 µs |  1.3× |

`is_empty` returns on the first non-zero word. Atomic `contains` loads the word with `Relaxed` ordering. Atomic `insert`, including the 65536-bit row, builds a fresh set inside the timed closure and sets one bit (`set_aliased` on `BitVec<AtomicU64, Lsb0>`).

Slower rows:

| Row                           |    Ours |  Theirs | Cause |
| ----------------------------- | ------: | ------: | ----- |
| 256-bit `index` vs bitvec     | 0.82 ns | 0.82 ns |       |
| 1024-bit `contains` vs bitvec | 0.82 ns | 0.82 ns |       |
| 1024-bit `index` vs bitvec    | 0.82 ns | 0.82 ns |       |

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

**Breaking:** out-of-range bit indices now panic in release builds.

An out-of-range bit index is a caller bug on every set type (primitive, array,
boxed, atomic, and `bitflagset!` / `atomic_bitflagset!`).

- `contains` / `insert` / `remove` / `set` / `toggle` / `Index` panic in debug
  and release with `index {idx} out of range for capacity {cap}`.
- Const `BitSet<primitive, usize>::contains`, const `from_index`, and const
  `bitflagset!` `contains` / position `from_element` use the literal
  `index out of range for capacity` because formatting is not available in
  `const fn`.
- `set[i]` (`Index`, output `&bool`) uses the element type as the key, so
  `set[Color::Red]` and `set[5usize]` both work. There is no `IndexMut`.
- `from_slice` and `from_bits_*` still accept untrusted input: unknown bits are
  dropped or rejected as before. Enum `from_element` still debug-asserts.

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
