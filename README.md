# bitflagset

Type-safe bitsets with `Set`-like ergonomics. Operations are direct primitive bit operations over words. Optional bitvec interop via the `bitvec` feature.

## Design philosophy

**Element-centric, not mask-centric.** Every type exposes a `HashSet`/`BTreeSet`-style interface — `contains(&V)`, `insert(V) -> bool`, `remove(V) -> bool`, `is_subset`, ... — while the underlying storage is a compact bit vector. The element type `V` is a generic parameter: an enum, a `usize`, or a named position constant. You work with domain values, never with raw bitmasks.

**Deref-based method sharing.** `BitSet`, `BoxedBitSet`, and array-backed `BitSet<[T; N], V>` all `Deref` to a shared unsized `BitSlice<T, V>` — a `#[repr(transparent)]` wrapper around `[T]`. Atomic types similarly share `AtomicBitSlice<A, V>`. Common methods are defined once on the slice type; owned types add storage-specific operations on top.

**Word-level primitive operations.** `BitSlice` operates on the raw `[T]` slice directly using `count_ones()`, bit masking, and word-level boolean operators.

**Const-friendly primitives.** For single-primitive `BitSet<u64, V>`, all query methods (`len`, `is_empty`, `is_subset`, ...) and constructors (`from_bits`, `from_index`, `from_indices`) are `const fn`. Build complex bitsets at compile time with zero runtime cost.

**Full bit-operator support.** Non-atomic owned types (`BitSet`, `BoxedBitSet`) implement `BitOr`, `BitAnd`, `BitXor`, `Not`, `Sub` and their `Assign` variants. Atomic types (`AtomicBitSet`, `AtomicBoxedBitSet`) expose equivalent set operations via named methods (`union`, `difference`, `includes`, etc.) since atomics are inherently `&self`-based.

## Performance

<!-- BENCH_TABLES:BEGIN -->
All numbers below are Criterion medians from `cargo bench --bench vs_bitvec` on Apple M-series (AArch64), collected on **2026-02-26**. Compared against bitvec `BitArray` (non-atomic) and `BitArray<AtomicU64>` / `BitVec<AtomicU64>` (atomic).  
`iter` rows measure `.iter().count()`.

### Non-atomic: `BitSet` vs bitvec `BitArray`

**256-bit** (`[u64; 4]`):

| Operation | bitflagset | bitvec | Speedup |
|-----------|-----------|--------|---------|
| `bitor` | 1.48 ns | 22.68 ns | **15.3x** |
| `bitand` | 1.48 ns | 22.62 ns | **15.3x** |
| `bitxor` | 1.48 ns | 22.63 ns | **15.3x** |
| `not` | 1.06 ns | 1.63 ns | **1.5x** |
| `iter` | 0.52 ns | 2.23 ns | **4.3x** |

**1024-bit** (`[u64; 16]`):

| Operation | bitflagset | bitvec | Speedup |
|-----------|-----------|--------|---------|
| `bitor` | 5.90 ns | 93.08 ns | **15.8x** |
| `bitand` | 5.88 ns | 94.39 ns | **16.1x** |
| `bitxor` | 6.44 ns | 94.61 ns | **14.7x** |
| `not` | 4.38 ns | 7.54 ns | **1.7x** |
| `iter` | 1.79 ns | 2.82 ns | **1.6x** |

Binary operators benefit from LLVM auto-vectorization of word-level loops into SIMD instructions.

### Atomic: `AtomicBitSet` vs bitvec `BitArray<AtomicU64>`

**256-bit** (`[AtomicU64; 4]`):

| Operation | bitflagset | bitvec | Speedup |
|-----------|-----------|--------|---------|
| `len` | 1.02 ns | 2.00 ns | **2.0x** |
| `is_empty` | 0.50 ns | 2.64 ns | **5.3x** |
| `contains` | 0.62 ns | 0.63 ns | **1.0x** |
| `insert` | 1.25 ns | 1.52 ns | **1.2x** |
| `iter` | 1.03 ns | 2.01 ns | **2.0x** |

**1024-bit** (`[AtomicU64; 16]`):

| Operation | bitflagset | bitvec | Speedup |
|-----------|-----------|--------|---------|
| `len` | 2.86 ns | 5.69 ns | **2.0x** |
| `is_empty` | 0.50 ns | 5.64 ns | **11.4x** |
| `contains` | 0.61 ns | 0.63 ns | **1.0x** |
| `insert` | 2.51 ns | 2.40 ns | **1.0x** |
| `iter` | 2.86 ns | 5.75 ns | **2.0x** |

**65536-bit** (heap-allocated, `AtomicBoxedBitSet` vs bitvec `BitVec<AtomicU64>`):

| Operation | bitflagset | bitvec | Speedup |
|-----------|-----------|--------|---------|
| `len` | 291.32 ns | 294.67 ns | **1.0x** |
| `is_empty` | 0.51 ns | 290.41 ns | **570.5x** |
| `contains` | 0.61 ns | 0.70 ns | **1.2x** |
| `iter` | 283.59 ns | 289.64 ns | **1.0x** |

`is_empty` uses short-circuit evaluation (early return on first non-zero word).
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
