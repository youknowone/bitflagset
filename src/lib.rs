#![no_std]

extern crate self as bitflagset;

#[cfg(feature = "alloc")]
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
}
