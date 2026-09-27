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
All numbers below are Criterion medians from `cargo bench --bench vs_bitvec` on Apple M-series (AArch64), collected on **2026-09-28**. Compared against bitvec `BitArray` (non-atomic) and `BitArray<AtomicU64>` / `BitVec<AtomicU64>` (atomic).  
`iter` rows measure `.iter().count()`.

### Non-atomic: `BitSet` vs bitvec `BitArray`

**256-bit** (`[u64; 4]`):

| Operation | bitflagset | bitvec | Speedup |
|-----------|-----------|--------|---------|
| `bitor` | 2.06 ns | 26.73 ns | **13.0x** |
| `bitand` | 2.01 ns | 28.65 ns | **14.3x** |
| `bitxor` | 2.10 ns | 26.76 ns | **12.7x** |
| `not` | 1.33 ns | 2.06 ns | **1.6x** |
| `iter` | 0.70 ns | 2.40 ns | **3.4x** |

**1024-bit** (`[u64; 16]`):

| Operation | bitflagset | bitvec | Speedup |
|-----------|-----------|--------|---------|
| `bitor` | 6.60 ns | 113.17 ns | **17.2x** |
| `bitand` | 6.74 ns | 110.64 ns | **16.4x** |
| `bitxor` | 6.44 ns | 114.71 ns | **17.8x** |
| `not` | 5.18 ns | 7.48 ns | **1.4x** |
| `iter` | 2.50 ns | 3.26 ns | **1.3x** |

Binary operators benefit from LLVM auto-vectorization of word-level loops into SIMD instructions.

### Atomic: `AtomicBitSet` vs bitvec `BitArray<AtomicU64>`

**256-bit** (`[AtomicU64; 4]`):

| Operation | bitflagset | bitvec | Speedup |
|-----------|-----------|--------|---------|
| `len` | 1.30 ns | 2.57 ns | **2.0x** |
| `is_empty` | 0.57 ns | 2.72 ns | **4.8x** |
| `contains` | 0.88 ns | 0.84 ns | **1.0x** |
| `insert` | 1.35 ns | 1.70 ns | **1.3x** |
| `iter` | 1.19 ns | 2.47 ns | **2.1x** |

**1024-bit** (`[AtomicU64; 16]`):

| Operation | bitflagset | bitvec | Speedup |
|-----------|-----------|--------|---------|
| `len` | 4.13 ns | 5.67 ns | **1.4x** |
| `is_empty` | 0.55 ns | 5.95 ns | **10.8x** |
| `contains` | 0.86 ns | 0.82 ns | **1.0x** |
| `insert` | 2.50 ns | 2.65 ns | **1.1x** |
| `iter` | 4.11 ns | 5.62 ns | **1.4x** |

`is_empty` uses short-circuit evaluation (early return on first non-zero word).

### vs bit-set / bit-vec

All numbers below are Criterion medians from `cargo bench --bench vs_bit_set` on Apple M-series (AArch64), collected on **2026-09-28**. `bit_set::BitSet` (default `u32` blocks) is pre-sized with `with_capacity`. `bit_vec::BitVec` (default `u32` blocks) is pre-sized with `from_elem`.  
`iter`, `union`, `intersection`, and `difference` rows measure iterator `.count()` with `iter`. `insert`, `remove`, `set`, `set_false`, `clear`, and the in-place rows (`union_with` / `intersect_with` / `difference_with`, `or` / `and` / `difference`) clone the destination in `iter_batched_ref` setup and time only the operation. `&=` / `-=` also clone the right-hand side in that setup (`iter_batched`), because those operators take it by value. Fixed-size rows use `BatchSize::SmallInput`; the 65536-bit rows use `BatchSize::LargeInput`. `len` is `len` / `count`. `clear` is `clear` / `make_empty`.

#### `BitSet<[u64; N]>` / `BoxedBitSet` vs `bit_set::BitSet`

**256-bit** (`[u64; 4]`):

| Operation | bitflagset | bit-set | Speedup |
|-----------|-----------|---------|---------|
| `insert` | 0.72 ns | 1.76 ns | **2.5x** |
| `remove` | 0.69 ns | 1.37 ns | **2.0x** |
| `contains` | 0.86 ns | 1.05 ns | **1.2x** |
| `len` | 0.78 ns | 2.13 ns | **2.7x** |
| `iter` | 0.75 ns | 2.78 ns | **3.7x** |
| `union` | 2.06 ns | 8.37 ns | **4.1x** |
| `intersection` | 2.17 ns | 8.00 ns | **3.7x** |
| `difference` | 2.06 ns | 8.14 ns | **4.0x** |
| `union_with` | 1.65 ns | 7.26 ns | **4.4x** |
| `intersect_with` | 1.38 ns | 6.98 ns | **5.1x** |
| `difference_with` | 1.37 ns | 7.28 ns | **5.3x** |
| `is_subset` | 0.63 ns | 1.20 ns | **1.9x** |
| `clear` | 1.79 ns | 3.07 ns | **1.7x** |

**1024-bit** (`[u64; 16]`):

| Operation | bitflagset | bit-set | Speedup |
|-----------|-----------|---------|---------|
| `insert` | 2.57 ns | 3.28 ns | **1.3x** |
| `remove` | 3.15 ns | 2.85 ns | **0.9x** |
| `contains` | 0.85 ns | 1.21 ns | **1.4x** |
| `len` | 2.73 ns | 3.67 ns | **1.3x** |
| `iter` | 3.14 ns | 6.88 ns | **2.2x** |
| `union` | 12.13 ns | 31.36 ns | **2.6x** |
| `intersection` | 9.31 ns | 36.48 ns | **3.9x** |
| `difference` | 9.26 ns | 33.82 ns | **3.7x** |
| `union_with` | 4.22 ns | 23.57 ns | **5.6x** |
| `intersect_with` | 10.63 ns | 23.78 ns | **2.2x** |
| `difference_with` | 10.85 ns | 23.34 ns | **2.2x** |
| `is_subset` | 0.74 ns | 1.29 ns | **1.7x** |
| `clear` | 2.36 ns | 3.00 ns | **1.3x** |

**65536-bit** (`BoxedBitSet`):

| Operation | bitflagset | bit-set | Speedup |
|-----------|-----------|---------|---------|
| `insert` | 4.04 ns | 4.95 ns | **1.2x** |
| `remove` | 4.82 ns | 2.05 ns | **0.4x** |
| `contains` | 0.97 ns | 1.22 ns | **1.3x** |
| `len` | 149.68 ns | 191.56 ns | **1.3x** |
| `iter` | 146.79 ns | 188.56 ns | **1.3x** |
| `union` | 696.64 ns | 1.78 us | **2.6x** |
| `intersection` | 640.16 ns | 1.82 us | **2.8x** |
| `difference` | 573.87 ns | 1.96 us | **3.4x** |
| `union_with` | 209.54 ns | 1.17 us | **5.6x** |
| `intersect_with` | 311.83 ns | 1.18 us | **3.8x** |
| `difference_with` | 306.94 ns | 1.17 us | **3.8x** |
| `is_subset` | 1.22 ns | 1.45 ns | **1.2x** |
| `clear` | 162.24 ns | 158.46 ns | **1.0x** |

`first` / `last` are omitted: `bit-set` has neither. Owned `|` / `&` / `-` are omitted: `bit-set` has no new-set operator. Iterator `union` / `intersection` / `difference` are the producing form; `union_with` / `intersect_with` / `difference_with` are in place (`union_from`, `&=`, `-=`).

#### `BitSet<[u64; N]>` / `BoxedBitSet` vs `bit_vec::BitVec`

**256-bit** (`[u64; 4]`):

| Operation | bitflagset | bit-vec | Speedup |
|-----------|-----------|---------|---------|
| `set` | 0.72 ns | 1.96 ns | **2.7x** |
| `set_false` | 0.72 ns | 1.92 ns | **2.7x** |
| `get` | 0.86 ns | 3.20 ns | **3.7x** |
| `count` | 0.78 ns | 2.11 ns | **2.7x** |
| `or` | 1.65 ns | 4.06 ns | **2.5x** |
| `and` | 1.38 ns | 3.95 ns | **2.9x** |
| `difference` | 1.37 ns | 3.86 ns | **2.8x** |
| `clear` | 1.79 ns | 3.24 ns | **1.8x** |

**1024-bit** (`[u64; 16]`):

| Operation | bitflagset | bit-vec | Speedup |
|-----------|-----------|---------|---------|
| `set` | 3.39 ns | 2.86 ns | **0.8x** |
| `set_false` | 1.96 ns | 3.49 ns | **1.8x** |
| `get` | 0.85 ns | 3.19 ns | **3.8x** |
| `count` | 2.73 ns | 3.69 ns | **1.4x** |
| `or` | 4.22 ns | 6.63 ns | **1.6x** |
| `and` | 10.63 ns | 6.71 ns | **0.6x** |
| `difference` | 10.85 ns | 7.06 ns | **0.7x** |
| `clear` | 2.36 ns | 2.91 ns | **1.2x** |

**65536-bit** (`BoxedBitSet`):

| Operation | bitflagset | bit-vec | Speedup |
|-----------|-----------|---------|---------|
| `set` | 4.36 ns | 4.59 ns | **1.1x** |
| `set_false` | 4.25 ns | 5.52 ns | **1.3x** |
| `get` | 0.97 ns | 3.21 ns | **3.3x** |
| `count` | 149.68 ns | 188.36 ns | **1.3x** |
| `or` | 209.54 ns | 212.06 ns | **1.0x** |
| `and` | 311.83 ns | 229.63 ns | **0.7x** |
| `difference` | 306.94 ns | 223.46 ns | **0.7x** |
| `clear` | 162.24 ns | 155.74 ns | **1.0x** |

`bit-vec` `insert` / `remove` shift the vector, so they are not compared. `clear` on the bit-vec side is `fill(false)`, which keeps the length. `bit-vec` has no `is_subset`, no set-index iterator, and no `first` / `last`.
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

All operators also have `Assign` variants (`|=`, `&=`, `^=`, `-=`).

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
