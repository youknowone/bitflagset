#![no_std]

extern crate self as bitflagset;

#[cfg(any(feature = "alloc", test))]
extern crate alloc;

mod atomic;
#[cfg(feature = "alloc")]
mod atomic_boxed;
mod atomic_slice;
mod bitset;
#[cfg(feature = "alloc")]
mod boxed;
mod enumset;
mod slice;
mod word_op;

pub use atomic::*;
#[cfg(feature = "alloc")]
pub use atomic_boxed::*;
pub use atomic_slice::*;
#[cfg(feature = "derive")]
pub use bitflagset_derive::{BitFlag, BitFlagSet};
pub use bitset::*;
#[cfg(feature = "alloc")]
pub use boxed::*;
pub use enumset::*;
pub use slice::*;

#[doc(hidden)]
pub mod __private {
    #[cfg(feature = "bitflags")]
    pub use bitflags;
    pub use radium;
    pub use ref_cast;

    #[cold]
    #[inline(never)]
    #[track_caller]
    pub fn panic_index_out_of_range(idx: usize, cap: usize) -> ! {
        panic!("index {idx} out of range for capacity {cap}");
    }

    /// Panic when `idx` is outside `0..cap`. In-range calls are one not-taken branch.
    #[inline(always)]
    pub fn check_bit_index(idx: usize, cap: usize) {
        if idx >= cap {
            panic_index_out_of_range(idx, cap);
        }
    }

    /// `&true` / `&false` for `Index<Output = bool>`.
    #[inline(always)]
    pub fn bit_ref(on: bool) -> &'static bool {
        static TABLE: [bool; 2] = [false, true];
        let idx = on as usize;
        // SAFETY: `bool as usize` is 0 or 1, and `TABLE` has length 2.
        unsafe { TABLE.get_unchecked(idx) }
    }
}
