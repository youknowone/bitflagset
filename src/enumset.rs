use num_traits::{One, PrimInt};

/// A named flag entry pairing a variant name with its value.
pub struct Flag<B> {
    name: &'static str,
    value: B,
}

impl<B> Flag<B> {
    #[inline]
    pub const fn new(name: &'static str, value: B) -> Self {
        Self { name, value }
    }

    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[inline]
    pub const fn value(&self) -> &B {
        &self.value
    }
}

/// A trait for types that represent individual bit flag positions.
///
/// Provides conversions between element values and their bitmask
/// representations, plus a static list of all defined flags.
///
/// Use the [`bitflag!`] macro to auto-derive this trait:
///
/// ```
/// bitflagset::bitflag! {
///     #[derive(Debug)]
///     #[repr(u8)]
///     enum Color {
///         Red = 0,
///         Green = 1,
///         Blue = 2,
///     }
/// }
///
/// use bitflagset::BitFlag;
/// assert_eq!(Color::FLAGS.len(), 3);
/// assert_eq!(Color::FLAGS[0].name(), "Red");
/// assert_eq!(Color::Red.mask(), 1u8);
/// assert_eq!(Color::Green.mask(), 2u8);
/// ```
pub trait BitFlag: Copy + Into<u8> + 'static {
    type Mask: PrimInt;
    const FLAGS: &'static [Flag<Self>];
    const MAX_VALUE: u8;

    #[inline]
    fn as_u8(self) -> u8 {
        self.into()
    }

    #[inline]
    fn as_usize(self) -> usize {
        self.into() as usize
    }

    #[inline]
    fn mask(self) -> Self::Mask {
        Self::Mask::one() << self.as_usize()
    }
}

pub trait BitFlagSet<T, A>
where
    Self: Sized
        + Copy
        + Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + core::ops::Sub<Self>
        + core::ops::SubAssign<Self>
        + core::ops::BitAnd<Self>
        + core::ops::BitAndAssign<Self>
        + core::ops::BitOr<Self>
        + core::ops::BitOrAssign<Self>
        + core::iter::FromIterator<T>
        + core::iter::FromIterator<Self>,
    A: PrimInt + core::ops::BitAndAssign<A>,
    T: TryFrom<u8>,
{
    const BITS: u8;
    fn empty() -> Self;
    fn from_bits_retain(raw: A) -> Self;
    fn from_element(element: T) -> Self;
    fn first(&self) -> Option<T>;
    fn last(&self) -> Option<T>;
    fn pop_first(&mut self) -> Option<T>;
    fn pop_last(&mut self) -> Option<T>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn contains(&self, value: &T) -> bool;
    fn retain(&mut self, f: impl FnMut(T) -> bool);
    fn insert(&mut self, value: T) -> bool;
    fn remove(&mut self, value: T) -> bool;
    fn is_subset(&self, other: &Self) -> bool;
    fn is_superset(&self, other: &Self) -> bool;
    fn is_disjoint(&self, other: &Self) -> bool;
    fn iter(&self) -> super::bitset::PrimBitSetIter<A, T>;
    #[cfg(feature = "alloc")]
    fn to_vec(&self) -> alloc::vec::Vec<T> {
        self.iter().collect()
    }
}

/// Defines a `#[repr(u8)]` enum and auto-implements [`BitFlag`],
/// `From<Enum> for u8`, and `TryFrom<u8>`.
///
/// ```
/// bitflagset::bitflag! {
///     #[derive(Debug)]
///     #[repr(u8)]
///     pub enum Color {
///         Red = 0,
///         Green = 1,
///         Blue = 2,
///     }
/// }
///
/// use bitflagset::BitFlag;
/// assert_eq!(Color::Red.mask(), 1u8);
/// assert_eq!(Color::Green.as_u8(), 1);
/// assert_eq!(Color::try_from(2), Ok(Color::Blue));
/// assert!(Color::try_from(99).is_err());
/// ```
#[macro_export]
macro_rules! bitflag {
    // With explicit discriminants
    (
        $(#[$outer:meta])*
        $vis:vis enum $name:ident {
            $($(#[$inner:meta])* $variant:ident = $value:expr),*
            $(,)?
        }
    ) => {
        $crate::bitflag!(@impl $(#[$outer])* $vis enum $name {
            $($(#[$inner])* $variant = $value),*
        });
    };

    // Without explicit discriminants (auto-assigned 0, 1, 2, ...)
    (
        $(#[$outer:meta])*
        $vis:vis enum $name:ident {
            $($(#[$inner:meta])* $variant:ident),*
            $(,)?
        }
    ) => {
        $crate::bitflag!(@impl $(#[$outer])* $vis enum $name {
            $($(#[$inner])* $variant),*
        });
    };

    (@impl
        $(#[$outer:meta])*
        $vis:vis enum $name:ident {
            $($(#[$inner:meta])* $variant:ident $(= $value:expr)?),*
        }
    ) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        $(#[$outer])*
        $vis enum $name {
            $($(#[$inner])* $variant $(= $value)?),*
        }

        const _: () = assert!(
            core::mem::size_of::<$name>() == core::mem::size_of::<u8>(),
            "bitflag! enum must use #[repr(u8)]"
        );

        impl From<$name> for u8 {
            #[inline]
            fn from(v: $name) -> u8 { v as u8 }
        }

        impl TryFrom<u8> for $name {
            type Error = ();
            fn try_from(v: u8) -> Result<Self, ()> {
                match v {
                    $(x if x == <$name>::$variant as u8 => Ok(<$name>::$variant),)*
                    _ => Err(()),
                }
            }
        }

        impl $crate::BitFlag for $name {
            type Mask = u8;
            const FLAGS: &'static [$crate::Flag<Self>] = &[
                $($crate::Flag::new(stringify!($variant), <$name>::$variant)),*
            ];
            const MAX_VALUE: u8 = {
                let mut max: u8 = 0;
                $(
                    let value = <$name>::$variant as u8;
                    if value > max {
                        max = value;
                    }
                )*
                max
            };
        }
    };
}

#[doc(hidden)]
#[macro_export]
#[cfg(feature = "bitflags")]
macro_rules! __bitflagset_impl_flags {
    ($name:ident, $repr:ty, [$(($flag_name:expr, $flag_value:expr)),* $(,)?]) => {
        impl $crate::__private::bitflags::Flags for $name {
            type Bits = $repr;

            const FLAGS: &'static [$crate::__private::bitflags::Flag<Self>] = &[
                $($crate::__private::bitflags::Flag::new($flag_name, $flag_value)),*
            ];

            #[inline]
            fn bits(&self) -> $repr {
                $name::bits(self)
            }

            #[inline]
            fn from_bits_retain(bits: $repr) -> Self {
                $name::from_bits_retain(bits)
            }
        }
    };
    ($name:ident, $repr:ty, bitflag: $typ:ty) => {
        impl $crate::__private::bitflags::Flags for $name {
            type Bits = $repr;

            const FLAGS: &'static [$crate::__private::bitflags::Flag<Self>] = &{
                const LEN: usize = <$typ as $crate::BitFlag>::FLAGS.len();
                let mut flags = [const {
                    $crate::__private::bitflags::Flag::new("", $name::empty())
                }; LEN];
                let mut i = 0;
                while i < LEN {
                    let flag = &<$typ as $crate::BitFlag>::FLAGS[i];
                    flags[i] = $crate::__private::bitflags::Flag::new(
                        flag.name(),
                        $name::from_element(*flag.value()),
                    );
                    i += 1;
                }
                flags
            };

            #[inline]
            fn bits(&self) -> $repr {
                $name::bits(self)
            }

            #[inline]
            fn from_bits_retain(bits: $repr) -> Self {
                $name::from_bits_retain(bits)
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
#[cfg(not(feature = "bitflags"))]
macro_rules! __bitflagset_impl_flags {
    ($($tt:tt)*) => {};
}

/// Generates a newtype bitset backed by a primitive integer.
///
/// Two forms are supported:
///
/// # Enum form
///
/// Wraps an existing `#[repr(u8)]` enum whose variants map 1:1 to bit
/// positions. The element type is the enum itself.
///
/// ```
/// # bitflagset::bitflag! {
/// #     #[derive(Debug)]
/// #     #[repr(u8)]
/// #     pub enum MyEnum {
/// #         A = 0,
/// #         B = 1,
/// #         C = 2,
/// #     }
/// # }
/// bitflagset::bitflagset!(pub struct MySet(u8) : MyEnum);
/// ```
///
/// Requirements:
/// * The enum must be `#[repr(u8)]` with discriminants that fit in the
///   storage primitive (e.g. 0..7 for `u8`, 0..63 for `u64`).
/// * `TryFrom<u8>` must be implemented for the enum.
/// * The enum must implement [`BitFlag`] so `FLAGS`/`MAX_VALUE` are available
///   for compile-time validation (`bitflag!` does this automatically).
/// * Using [`bitflag!`] for the enum is optional but convenient. If you do not
///   use it, define the enum manually and implement the required conversions
///   and traits yourself.
///
/// # Position form (bitflags-compatible)
///
/// Defines named flag constants directly inside the struct, matching
/// the `bitflags!` macro syntax. The element type is the struct itself.
///
/// ```
/// bitflagset::bitflagset! {
///     pub struct MyFlags(u8) {
///         const A = 0;
///         const B = 1;
///         const C = 2;
///     }
/// }
///
/// let mut f = MyFlags::empty();
/// f.insert(MyFlags::A);
/// f.insert(MyFlags::B);
/// assert!(f.contains(&MyFlags::A));
/// assert_eq!(f.len(), 2);
/// ```
///
/// When the `bitflags` feature is enabled, the generated struct also
/// implements `bitflags::Flags`.
#[macro_export]
macro_rules! bitflagset {
    // Shared operator impls
    (@ops $name:ident, $repr:ty) => {
        impl core::ops::Sub<$name> for $name {
            type Output = $name;

            #[inline]
            fn sub(self, rhs: $name) -> Self::Output {
                Self(self.0 & !rhs.0)
            }
        }

        impl core::ops::SubAssign<$name> for $name {
            #[inline]
            fn sub_assign(&mut self, rhs: $name) {
                self.0 &= !rhs.0;
            }
        }

        impl core::ops::BitAnd<$name> for $name {
            type Output = $name;

            #[inline]
            fn bitand(self, rhs: $name) -> Self::Output {
                Self(self.0 & rhs.0)
            }
        }

        impl core::ops::BitAndAssign<$name> for $name {
            #[inline]
            fn bitand_assign(&mut self, rhs: $name) {
                self.0 &= rhs.0;
            }
        }

        impl core::ops::BitOr<$name> for $name {
            type Output = $name;

            #[inline]
            fn bitor(self, rhs: $name) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }

        impl core::ops::BitOrAssign<$name> for $name {
            #[inline]
            fn bitor_assign(&mut self, rhs: $name) {
                self.0 |= rhs.0;
            }
        }

        impl core::ops::BitXor<$name> for $name {
            type Output = $name;

            #[inline]
            fn bitxor(self, rhs: $name) -> Self::Output {
                Self(self.0 ^ rhs.0)
            }
        }

        impl core::ops::BitXorAssign<$name> for $name {
            #[inline]
            fn bitxor_assign(&mut self, rhs: $name) {
                self.0 ^= rhs.0;
            }
        }

        impl core::fmt::Binary for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::Binary::fmt(&self.0, f)
            }
        }

        impl core::fmt::Octal for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::Octal::fmt(&self.0, f)
            }
        }

        impl core::fmt::LowerHex for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::LowerHex::fmt(&self.0, f)
            }
        }

        impl core::fmt::UpperHex for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::UpperHex::fmt(&self.0, f)
            }
        }
    };

    // Enum form: enum-backed
    ($vis:vis struct $name:ident($repr:ty) : $typ:ty) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        $vis struct $name($crate::BitSet<$repr, u8>);

        const _: () = assert!(
            <$typ as $crate::BitFlag>::MAX_VALUE < <$repr>::BITS as u8,
            "bitflagset! enum discriminant exceeds storage width"
        );

        impl Default for $name {
            #[inline]
            fn default() -> Self {
                Self::new()
            }
        }

        impl core::fmt::Debug for $name
        where
            $typ: core::fmt::Debug + TryFrom<u8>,
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&format_args!("0x{:x}", self.bits()))
                    .finish()?;
                write!(f, "/* elements: [")?;
                let mut first = true;
                for elem in self.iter() {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "{elem:?}")?;
                    first = false;
                }
                write!(f, "] */")
            }
        }

        #[allow(dead_code)]
        impl $name {
            #[inline]
            pub const fn new() -> Self {
                Self($crate::BitSet::<$repr, u8>::from_bits(0))
            }
            #[inline]
            pub const fn empty() -> Self {
                Self::new()
            }
            #[inline]
            pub const fn from_bits_retain(raw: $repr) -> Self {
                Self($crate::BitSet::<$repr, u8>::from_bits(raw))
            }
            #[inline]
            pub const fn bits(&self) -> $repr {
                self.0.bits()
            }
            #[inline]
            pub const unsafe fn from_bits_unchecked(bits: $repr) -> Self {
                Self($crate::BitSet::<$repr, u8>::from_bits(bits))
            }
            #[inline]
            pub fn from_bits(bits: $repr) -> Option<Self>
            where
                $typ: TryFrom<u8>,
            {
                if bits & !Self::all().bits() == 0 {
                    Some(Self::from_bits_retain(bits))
                } else {
                    None
                }
            }
            #[inline]
            pub fn from_bits_truncate(bits: $repr) -> Self
            where
                $typ: TryFrom<u8>,
            {
                Self::from_bits_retain(bits & Self::all().bits())
            }
            #[inline]
            pub const fn from_element(element: $typ) -> Self {
                let shift = element as u8;
                debug_assert!(shift < <Self as $crate::BitFlagSet<$typ, $repr>>::BITS);
                Self::from_bits_retain((1 as $repr) << shift)
            }
            #[inline]
            pub const fn from_slice(slice: &[$typ]) -> Self {
                let mut raw: $repr = 0;
                let mut idx = 0;
                while idx < slice.len() {
                    let value = slice[idx];
                    let shift = value as u8;
                    debug_assert!(shift < <Self as $crate::BitFlagSet<$typ, $repr>>::BITS);
                    raw |= (1 as $repr) << shift;
                    idx += 1;
                }
                Self::from_bits_retain(raw)
            }
            #[inline]
            pub const fn contains(&self, value: &$typ) -> bool {
                let shift = *value as u8;
                debug_assert!(shift < <Self as $crate::BitFlagSet<$typ, $repr>>::BITS);
                self.bits() & ((1 as $repr) << shift) != 0
            }
            #[inline]
            pub const fn len(&self) -> usize {
                self.bits().count_ones() as usize
            }
            #[inline]
            pub const fn is_empty(&self) -> bool {
                self.bits() == 0
            }
            #[inline]
            pub fn all() -> Self {
                let mut mask: $repr = 0;
                let mut i: u8 = 0;
                while i < <$repr>::BITS as u8 {
                    if <$typ>::try_from(i).is_ok() {
                        mask |= (1 as $repr) << i;
                    }
                    i += 1;
                }
                Self::from_bits_retain(mask)
            }
            #[inline]
            pub fn is_all(&self) -> bool {
                self.bits() == Self::all().bits()
            }
            #[inline]
            pub fn complement(self) -> Self {
                Self::from_bits_retain(Self::all().bits() & !self.bits())
            }
            #[inline]
            pub fn toggle(&mut self, value: $typ) {
                let shift = value as u8;
                debug_assert!(shift < <Self as $crate::BitFlagSet<$typ, $repr>>::BITS);
                self.0.toggle(shift);
            }
            #[inline]
            pub fn set(&mut self, value: $typ, enabled: bool) {
                let shift = value as u8;
                debug_assert!(shift < <Self as $crate::BitFlagSet<$typ, $repr>>::BITS);
                self.0.set(shift, enabled);
            }
            #[inline]
            pub fn insert(&mut self, value: $typ) -> bool {
                let shift = value as u8;
                debug_assert!(shift < <Self as $crate::BitFlagSet<$typ, $repr>>::BITS);
                self.0.insert(shift)
            }
            #[inline]
            pub fn remove(&mut self, value: $typ) -> bool {
                let shift = value as u8;
                debug_assert!(shift < <Self as $crate::BitFlagSet<$typ, $repr>>::BITS);
                self.0.remove(shift)
            }
            #[inline]
            pub fn clear(&mut self) {
                self.0 = $crate::BitSet::<$repr, u8>::from_bits(0);
            }
            #[inline]
            pub const fn is_subset(&self, other: &Self) -> bool {
                (self.bits() & other.bits()) == self.bits()
            }
            #[inline]
            pub const fn is_superset(&self, other: &Self) -> bool {
                (self.bits() & other.bits()) == other.bits()
            }
            #[inline]
            pub const fn is_disjoint(&self, other: &Self) -> bool {
                (self.bits() & other.bits()) == 0
            }
            #[inline]
            pub fn first(&self) -> Option<$typ>
            where
                $typ: TryFrom<u8>,
            {
                let bits = self.bits();
                if bits == 0 {
                    return None;
                }
                let idx = bits.trailing_zeros() as u8;
                let converted = idx.try_into();
                debug_assert!(converted.is_ok());
                Some(match converted {
                    Ok(value) => value,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                })
            }
            #[inline]
            pub fn last(&self) -> Option<$typ>
            where
                $typ: TryFrom<u8>,
            {
                let bits = self.bits();
                if bits == 0 {
                    return None;
                }
                let idx = (<$repr>::BITS - 1 - bits.leading_zeros()) as u8;
                let converted = idx.try_into();
                debug_assert!(converted.is_ok());
                Some(match converted {
                    Ok(value) => value,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                })
            }
            #[inline]
            pub fn pop_first(&mut self) -> Option<$typ>
            where
                $typ: TryFrom<u8>,
            {
                let mut bits = self.bits();
                while bits != 0 {
                    let idx = bits.trailing_zeros() as u8;
                    let mask = (1 as $repr) << idx;
                    bits &= !mask;
                    if let Ok(value) = idx.try_into() {
                        self.0 = $crate::BitSet::<$repr, u8>::from_bits(self.bits() & !mask);
                        return Some(value);
                    }
                }
                None
            }
            #[inline]
            pub fn pop_last(&mut self) -> Option<$typ>
            where
                $typ: TryFrom<u8>,
            {
                let mut bits = self.bits();
                while bits != 0 {
                    let idx = (<$repr>::BITS - 1 - bits.leading_zeros()) as u8;
                    let mask = (1 as $repr) << idx;
                    bits &= !mask;
                    if let Ok(value) = idx.try_into() {
                        self.0 = $crate::BitSet::<$repr, u8>::from_bits(self.bits() & !mask);
                        return Some(value);
                    }
                }
                None
            }
            #[inline]
            pub fn retain(&mut self, mut f: impl FnMut($typ) -> bool)
            where
                $typ: TryFrom<u8>,
            {
                let mut raw = self.bits();
                let mut bits = raw;
                while bits != 0 {
                    let idx = bits.trailing_zeros() as u8;
                    let mask = (1 as $repr) << idx;
                    bits &= !mask;
                    if let Ok(value) = <$typ>::try_from(idx) {
                        if !f(value) {
                            raw &= !mask;
                        }
                    }
                }
                self.0 = $crate::BitSet::<$repr, u8>::from_bits(raw);
            }
            #[inline]
            pub fn iter(&self) -> $crate::PrimBitSetIter<$repr, $typ> {
                $crate::PrimBitSetIter::from_raw(self.bits())
            }

            #[inline]
            pub fn iter_names(&self) -> impl Iterator<Item = (&'static str, $typ)> + '_
            where
                $typ: $crate::BitFlag,
            {
                <$typ as $crate::BitFlag>::FLAGS.iter().filter_map(move |flag| {
                    let value = *flag.value();
                    if self.contains(&value) {
                        Some((flag.name(), value))
                    } else {
                        None
                    }
                })
            }
        }

        impl<const N: usize> From<[$typ; N]> for $name {
            #[inline]
            fn from(array: [$typ; N]) -> Self {
                Self::from_slice(&array)
            }
        }

        impl From<$name> for $crate::BitSet<$repr, $typ> {
            #[inline]
            fn from(val: $name) -> Self {
                Self::from_bits(val.bits())
            }
        }

        impl From<$crate::BitSet<$repr, $typ>> for $name {
            #[inline]
            fn from(bs: $crate::BitSet<$repr, $typ>) -> Self {
                Self::from_bits_retain(bs.bits())
            }
        }

        $crate::bitflagset!(@ops $name, $repr);

        impl core::ops::Not for $name {
            type Output = $name;

            #[inline]
            fn not(self) -> Self::Output {
                self.complement()
            }
        }

        impl core::iter::Extend<$typ> for $name {
            #[inline]
            fn extend<I: IntoIterator<Item = $typ>>(&mut self, iter: I) {
                for value in iter {
                    self.insert(value);
                }
            }
        }

        impl core::iter::IntoIterator for $name {
            type Item = $typ;
            type IntoIter = $crate::PrimBitSetIter<$repr, $typ>;

            #[inline]
            fn into_iter(self) -> Self::IntoIter {
                self.iter()
            }
        }

        impl core::iter::FromIterator<$typ> for $name {
            #[inline]
            fn from_iter<I: IntoIterator<Item = $typ>>(iter: I) -> Self {
                let mut raw: $repr = 0;
                for value in iter {
                    raw |= (1 as $repr) << (value as usize);
                }
                Self::from_bits_retain(raw)
            }
        }

        impl core::iter::FromIterator<$name> for $name {
            #[inline]
            fn from_iter<I: IntoIterator<Item = $name>>(iter: I) -> Self {
                let mut raw: $repr = 0;
                for bitset in iter {
                    raw |= bitset.bits();
                }
                Self::from_bits_retain(raw)
            }
        }

        impl $crate::BitFlagSet<$typ, $repr> for $name {
            const BITS: u8 = <$repr>::BITS as u8;
            #[inline]
            fn empty() -> Self {
                Self::empty()
            }
            #[inline]
            fn from_bits_retain(raw: $repr) -> Self {
                Self::from_bits_retain(raw)
            }
            #[inline]
            fn from_element(element: $typ) -> Self {
                Self::from_element(element)
            }
            #[inline]
            fn first(&self) -> Option<$typ> {
                $name::first(self)
            }
            #[inline]
            fn last(&self) -> Option<$typ> {
                $name::last(self)
            }
            #[inline]
            fn pop_first(&mut self) -> Option<$typ> {
                $name::pop_first(self)
            }
            #[inline]
            fn pop_last(&mut self) -> Option<$typ> {
                $name::pop_last(self)
            }
            #[inline]
            fn len(&self) -> usize {
                $name::len(self)
            }
            #[inline]
            fn is_empty(&self) -> bool {
                $name::is_empty(self)
            }
            #[inline]
            fn insert(&mut self, value: $typ) -> bool {
                $name::insert(self, value)
            }
            #[inline]
            fn remove(&mut self, value: $typ) -> bool {
                $name::remove(self, value)
            }
            #[inline]
            fn is_subset(&self, other: &Self) -> bool {
                $name::is_subset(self, other)
            }
            #[inline]
            fn is_superset(&self, other: &Self) -> bool {
                $name::is_superset(self, other)
            }
            #[inline]
            fn is_disjoint(&self, other: &Self) -> bool {
                $name::is_disjoint(self, other)
            }
            #[inline]
            fn contains(&self, value: &$typ) -> bool {
                $name::contains(self, value)
            }
            #[inline]
            fn retain(&mut self, f: impl FnMut($typ) -> bool) {
                $name::retain(self, f)
            }
            #[inline]
            fn iter(&self) -> $crate::PrimBitSetIter<$repr, $typ> {
                $name::iter(self)
            }
        }

        $crate::__bitflagset_impl_flags!($name, $repr, bitflag: $typ);
    };

    // Position form: bitflags-compatible (no enum)
    ($vis:vis struct $name:ident($repr:ty) {
        $($(#[$inner:meta])* const $flag:ident = $value:expr;)*
    }) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $vis struct $name($crate::BitSet<$repr, u8>);

        $(
            const _: () = assert!(
                $value < <$repr>::BITS as u8,
                "bitflagset! position constant exceeds storage width"
            );
        )*

        impl Default for $name {
            #[inline]
            fn default() -> Self {
                Self::new()
            }
        }

        #[allow(dead_code, non_upper_case_globals)]
        impl $name {
            // Position constants (element values, not shifted masks)
            $(
                $(#[$inner])*
                pub const $flag: u8 = $value;
            )*

            const ALL_MASK: $repr = 0 $(| ((1 as $repr) << $value))*;

            #[inline]
            pub const fn new() -> Self {
                Self($crate::BitSet::<$repr, u8>::from_bits(0))
            }

            #[inline]
            pub const fn empty() -> Self {
                Self::new()
            }

            #[inline]
            pub const fn all() -> Self {
                Self($crate::BitSet::<$repr, u8>::from_bits(Self::ALL_MASK))
            }

            #[inline]
            pub const fn bits(&self) -> $repr {
                self.0.bits()
            }

            #[inline]
            pub const fn from_element(pos: u8) -> Self {
                debug_assert!(pos < <Self as $crate::BitFlagSet<u8, $repr>>::BITS);
                if pos < <Self as $crate::BitFlagSet<u8, $repr>>::BITS {
                    Self::from_bits_retain((1 as $repr) << pos)
                } else {
                    Self::empty()
                }
            }

            #[inline]
            pub const fn from_slice(positions: &[u8]) -> Self {
                let mut raw: $repr = 0;
                let mut i = 0;
                while i < positions.len() {
                    let pos = positions[i];
                    debug_assert!(pos < <Self as $crate::BitFlagSet<u8, $repr>>::BITS);
                    if pos < <Self as $crate::BitFlagSet<u8, $repr>>::BITS {
                        raw |= (1 as $repr) << pos;
                    }
                    i += 1;
                }
                Self::from_bits_retain(raw)
            }

            #[inline]
            pub const fn from_bits(bits: $repr) -> Option<Self> {
                if bits & !Self::ALL_MASK == 0 {
                    Some(Self::from_bits_retain(bits))
                } else {
                    None
                }
            }

            #[inline]
            pub const fn from_bits_retain(bits: $repr) -> Self {
                Self($crate::BitSet::<$repr, u8>::from_bits(bits))
            }

            #[inline]
            pub const unsafe fn from_bits_unchecked(bits: $repr) -> Self {
                Self($crate::BitSet::<$repr, u8>::from_bits(bits))
            }

            #[inline]
            pub const fn from_bits_truncate(bits: $repr) -> Self {
                Self::from_bits_retain(bits & Self::ALL_MASK)
            }

            #[inline]
            pub const fn len(&self) -> usize {
                self.bits().count_ones() as usize
            }

            #[inline]
            pub const fn is_empty(&self) -> bool {
                self.bits() == 0
            }

            #[inline]
            pub const fn is_all(&self) -> bool {
                self.bits() & Self::ALL_MASK == Self::ALL_MASK
            }

            #[inline]
            pub const fn contains(&self, pos: &u8) -> bool {
                debug_assert!(*pos < <Self as $crate::BitFlagSet<u8, $repr>>::BITS);
                *pos < <Self as $crate::BitFlagSet<u8, $repr>>::BITS
                    && (self.bits() & ((1 as $repr) << *pos) != 0)
            }

            #[inline]
            pub fn set(&mut self, pos: u8, value: bool) {
                self.0.set(pos, value);
            }

            #[inline]
            pub fn insert(&mut self, pos: u8) -> bool {
                self.0.insert(pos)
            }

            #[inline]
            pub fn remove(&mut self, pos: u8) -> bool {
                self.0.remove(pos)
            }

            #[inline]
            pub fn toggle(&mut self, pos: u8) {
                self.0.toggle(pos);
            }

            #[inline]
            pub fn clear(&mut self) {
                self.0 = $crate::BitSet::<$repr, u8>::from_bits(0);
            }

            #[inline]
            pub const fn is_subset(&self, other: &Self) -> bool {
                (self.bits() & other.bits()) == self.bits()
            }

            #[inline]
            pub const fn is_superset(&self, other: &Self) -> bool {
                (self.bits() & other.bits()) == other.bits()
            }

            #[inline]
            pub const fn is_disjoint(&self, other: &Self) -> bool {
                (self.bits() & other.bits()) == 0
            }

            #[inline]
            pub const fn complement(self) -> Self {
                Self::from_bits_retain(Self::ALL_MASK & !self.bits())
            }

            #[inline]
            pub fn iter(&self) -> $crate::PrimBitSetIter<$repr, u8> {
                $crate::PrimBitSetIter::from_raw(self.bits())
            }

            #[inline]
            pub fn first(&self) -> Option<u8> {
                let bits = self.bits();
                if bits == 0 {
                    None
                } else {
                    Some(bits.trailing_zeros() as u8)
                }
            }

            #[inline]
            pub fn last(&self) -> Option<u8> {
                let bits = self.bits();
                if bits == 0 {
                    None
                } else {
                    Some((<$repr>::BITS - 1 - bits.leading_zeros()) as u8)
                }
            }

            #[inline]
            pub fn pop_first(&mut self) -> Option<u8> {
                let bits = self.bits();
                if bits == 0 {
                    None
                } else {
                    let idx = bits.trailing_zeros() as u8;
                    self.0 = $crate::BitSet::<$repr, u8>::from_bits(bits & !((1 as $repr) << idx));
                    Some(idx)
                }
            }

            #[inline]
            pub fn pop_last(&mut self) -> Option<u8> {
                let bits = self.bits();
                if bits == 0 {
                    None
                } else {
                    let idx = (<$repr>::BITS - 1 - bits.leading_zeros()) as u8;
                    self.0 = $crate::BitSet::<$repr, u8>::from_bits(bits & !((1 as $repr) << idx));
                    Some(idx)
                }
            }

            pub fn retain(&mut self, mut f: impl FnMut(u8) -> bool) {
                let mut raw = self.bits();
                let mut bits = raw;
                while bits != 0 {
                    let idx = bits.trailing_zeros() as u8;
                    let mask = (1 as $repr) << idx;
                    bits &= !mask;
                    if !f(idx) {
                        raw &= !mask;
                    }
                }
                self.0 = $crate::BitSet::<$repr, u8>::from_bits(raw);
            }

            pub fn iter_names(&self) -> impl Iterator<Item = (&'static str, u8)> {
                let bits = self.bits();
                [
                    $((stringify!($flag), $name::$flag, (1 as $repr) << $name::$flag)),*
                ].into_iter()
                .filter(move |(_, _, mask)| bits & *mask != 0)
                .map(|(name, pos, _)| (name, pos))
            }
        }

        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                let mut remaining = self.bits();
                write!(f, "{}(", stringify!($name))?;
                let mut first = true;
                $(
                    {
                        let mask: $repr = (1 as $repr) << $name::$flag;
                        if remaining & mask != 0 {
                            if !first { write!(f, " | ")?; }
                            write!(f, "{}", stringify!($flag))?;
                            remaining &= !mask;
                            first = false;
                        }
                    }
                )*
                if remaining != 0 {
                    if !first { write!(f, " | ")?; }
                    write!(f, "0x{:x}", remaining)?;
                } else if first {
                    write!(f, "empty")?;
                }
                write!(f, ")")
            }
        }

        $crate::bitflagset!(@ops $name, $repr);

        impl core::ops::Not for $name {
            type Output = $name;

            #[inline]
            fn not(self) -> Self::Output {
                self.complement()
            }
        }

        impl core::iter::IntoIterator for $name {
            type Item = u8;
            type IntoIter = $crate::PrimBitSetIter<$repr, u8>;

            #[inline]
            fn into_iter(self) -> Self::IntoIter {
                self.iter()
            }
        }

        impl core::iter::FromIterator<u8> for $name {
            #[inline]
            fn from_iter<I: IntoIterator<Item = u8>>(iter: I) -> Self {
                let mut raw: $repr = 0;
                for pos in iter {
                    raw |= (1 as $repr) << pos;
                }
                Self::from_bits_retain(raw)
            }
        }

        impl core::iter::Extend<u8> for $name {
            #[inline]
            fn extend<I: IntoIterator<Item = u8>>(&mut self, iter: I) {
                for pos in iter {
                    self.0.insert(pos);
                }
            }
        }

        impl core::iter::FromIterator<$name> for $name {
            #[inline]
            fn from_iter<I: IntoIterator<Item = $name>>(iter: I) -> Self {
                let mut raw: $repr = 0;
                for set in iter {
                    raw |= set.bits();
                }
                Self::from_bits_retain(raw)
            }
        }

        impl core::iter::Extend<$name> for $name {
            #[inline]
            fn extend<I: IntoIterator<Item = $name>>(&mut self, iter: I) {
                for set in iter {
                    self.0.union_from(set.bits());
                }
            }
        }

        impl $crate::BitFlagSet<u8, $repr> for $name {
            const BITS: u8 = <$repr>::BITS as u8;

            #[inline]
            fn empty() -> Self {
                Self::empty()
            }

            #[inline]
            fn from_bits_retain(raw: $repr) -> Self {
                Self::from_bits_retain(raw)
            }

            #[inline]
            fn from_element(element: u8) -> Self {
                Self::from_element(element)
            }

            #[inline]
            fn first(&self) -> Option<u8> {
                $name::first(self)
            }

            #[inline]
            fn last(&self) -> Option<u8> {
                $name::last(self)
            }

            #[inline]
            fn pop_first(&mut self) -> Option<u8> {
                $name::pop_first(self)
            }

            #[inline]
            fn pop_last(&mut self) -> Option<u8> {
                $name::pop_last(self)
            }

            #[inline]
            fn len(&self) -> usize {
                $name::len(self)
            }

            #[inline]
            fn is_empty(&self) -> bool {
                $name::is_empty(self)
            }

            #[inline]
            fn contains(&self, value: &u8) -> bool {
                $name::contains(self, value)
            }

            #[inline]
            fn retain(&mut self, f: impl FnMut(u8) -> bool) {
                $name::retain(self, f)
            }

            #[inline]
            fn insert(&mut self, value: u8) -> bool {
                $name::insert(self, value)
            }

            #[inline]
            fn remove(&mut self, value: u8) -> bool {
                $name::remove(self, value)
            }

            #[inline]
            fn is_subset(&self, other: &Self) -> bool {
                $name::is_subset(self, other)
            }

            #[inline]
            fn is_superset(&self, other: &Self) -> bool {
                $name::is_superset(self, other)
            }

            #[inline]
            fn is_disjoint(&self, other: &Self) -> bool {
                $name::is_disjoint(self, other)
            }

            #[inline]
            fn iter(&self) -> $crate::PrimBitSetIter<$repr, u8> {
                $name::iter(self)
            }
        }

        $crate::__bitflagset_impl_flags!(
            $name,
            $repr,
            [$((
                stringify!($flag),
                $name::from_element($name::$flag)
            )),*]
        );
    };
}

/// Generates an atomic bitset wrapper over [`AtomicBitSet`].
///
/// Two forms are supported, mirroring [`bitflagset!`]:
///
/// - Enum form:
///   `atomic_bitflagset!(pub struct AtomicMySet(AtomicU8) on MySet);`
/// - Position form:
///   `atomic_bitflagset! { pub struct AtomicMyFlags(AtomicU8) on MyFlags { const A = 0; } }`
///
/// Methods that mutate bits take `&self` (atomic interior mutability).
#[macro_export]
macro_rules! atomic_bitflagset {
    // Preferred syntax: enum/typed set form
    ($vis:vis struct $name:ident($atomic:ty) on $set:ty) => {
        $crate::atomic_bitflagset!(
            @enum_impl
            $vis struct $name($atomic, $set) : <$set as core::iter::IntoIterator>::Item
        );
    };

    // Preferred syntax: position form
    ($vis:vis struct $name:ident($atomic:ty) on $set:ty {
        $($(#[$inner:meta])* const $flag:ident = $value:expr;)*
    }) => {
        $crate::atomic_bitflagset! {
            @position_impl
            $vis struct $name($atomic, $set) {
                $($(#[$inner])* const $flag = $value;)*
            }
        }
    };

    // Internal enum form implementation
    (@enum_impl $vis:vis struct $name:ident($atomic:ty, $set:ty) : $typ:ty) => {
        $vis struct $name($crate::AtomicBitSet<$atomic, u8>);

        const _: () = assert!(
            <$typ as $crate::BitFlag>::MAX_VALUE < (core::mem::size_of::<$atomic>() * 8) as u8,
            "atomic_bitflagset! enum discriminant exceeds storage width"
        );

        impl Default for $name {
            #[inline]
            fn default() -> Self {
                Self::new()
            }
        }

        impl $name {
            #[inline]
            fn all_mask() -> <$atomic as $crate::__private::radium::Radium>::Item {
                let mut mask: <$atomic as $crate::__private::radium::Radium>::Item = 0;
                let mut i = 0usize;
                while i < <$typ as $crate::BitFlag>::FLAGS.len() {
                    let shift = *<$typ as $crate::BitFlag>::FLAGS[i].value() as u8;
                    mask |= (1 as <$atomic as $crate::__private::radium::Radium>::Item) << shift;
                    i += 1;
                }
                mask
            }

            #[inline]
            fn from_raw(bits: <$atomic as $crate::__private::radium::Radium>::Item) -> Self {
                Self($crate::AtomicBitSet::<$atomic, u8>::from_bits(<$atomic>::new(bits)))
            }

            #[inline]
            pub const fn new() -> Self {
                Self($crate::AtomicBitSet::<$atomic, u8>::new())
            }

            #[inline]
            pub const fn empty() -> Self {
                Self::new()
            }

            #[inline]
            pub fn bits(&self) -> <$atomic as $crate::__private::radium::Radium>::Item {
                self.0.load_store()
            }

            #[inline]
            pub unsafe fn from_bits_unchecked(bits: <$atomic as $crate::__private::radium::Radium>::Item) -> Self {
                Self::from_raw(bits)
            }

            #[inline]
            pub fn from_bits(bits: <$atomic as $crate::__private::radium::Radium>::Item) -> Option<Self> {
                if bits & !Self::all_mask() == 0 {
                    Some(Self::from_raw(bits))
                } else {
                    None
                }
            }

            #[inline]
            pub fn from_bits_truncate(bits: <$atomic as $crate::__private::radium::Radium>::Item) -> Self {
                Self::from_raw(bits & Self::all_mask())
            }

            #[inline]
            pub fn from_plain(set: $set) -> Self {
                let bits: <$atomic as $crate::__private::radium::Radium>::Item = set.bits();
                Self::from_raw(bits)
            }

            #[inline]
            pub fn into_plain(self) -> $set {
                <$set>::from_bits_retain(self.bits())
            }

            #[inline]
            pub fn swap_bits(&self, bits: &mut <$atomic as $crate::__private::radium::Radium>::Item) {
                self.0.swap_store(bits);
            }

            #[inline]
            pub fn from_element(element: $typ) -> Self {
                let shift = element as u8;
                debug_assert!(shift < (core::mem::size_of::<$atomic>() * 8) as u8);
                Self::from_raw((1 as <$atomic as $crate::__private::radium::Radium>::Item) << shift)
            }

            #[inline]
            pub fn from_slice(slice: &[$typ]) -> Self {
                let mut raw: <$atomic as $crate::__private::radium::Radium>::Item = 0;
                for &value in slice {
                    let shift = value as u8;
                    debug_assert!(shift < (core::mem::size_of::<$atomic>() * 8) as u8);
                    raw |= (1 as <$atomic as $crate::__private::radium::Radium>::Item) << shift;
                }
                Self::from_raw(raw)
            }

            #[inline]
            pub fn all() -> Self {
                Self::from_raw(Self::all_mask())
            }

            #[inline]
            pub fn is_all(&self) -> bool {
                self.bits() == Self::all_mask()
            }

            #[inline]
            pub fn complement(&self) -> Self {
                Self::from_raw(Self::all_mask() & !self.bits())
            }

            #[inline]
            pub fn len(&self) -> usize {
                self.0.len()
            }

            #[inline]
            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }

            #[inline]
            pub fn contains(&self, value: &$typ) -> bool {
                let shift = *value as u8;
                debug_assert!(shift < (core::mem::size_of::<$atomic>() * 8) as u8);
                self.0.contains(&shift)
            }

            #[inline]
            pub fn set(&self, value: $typ, enabled: bool) {
                let shift = value as u8;
                debug_assert!(shift < (core::mem::size_of::<$atomic>() * 8) as u8);
                self.0.set(shift, enabled);
            }

            #[inline]
            pub fn insert(&self, value: $typ) -> bool {
                let shift = value as u8;
                debug_assert!(shift < (core::mem::size_of::<$atomic>() * 8) as u8);
                self.0.insert(shift)
            }

            #[inline]
            pub fn remove(&self, value: $typ) -> bool {
                let shift = value as u8;
                debug_assert!(shift < (core::mem::size_of::<$atomic>() * 8) as u8);
                self.0.remove(shift)
            }

            #[inline]
            pub fn toggle(&self, value: $typ) {
                let shift = value as u8;
                debug_assert!(shift < (core::mem::size_of::<$atomic>() * 8) as u8);
                self.0.toggle(shift);
            }

            #[inline]
            pub fn clear(&self) {
                self.0.clear();
            }

            #[inline]
            pub fn first(&self) -> Option<$typ>
            where
                $typ: TryFrom<u8>,
            {
                let raw = self.0.first()?;
                let converted = <$typ>::try_from(raw);
                debug_assert!(converted.is_ok());
                Some(match converted {
                    Ok(value) => value,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                })
            }

            #[inline]
            pub fn last(&self) -> Option<$typ>
            where
                $typ: TryFrom<u8>,
            {
                let raw = self.0.last()?;
                let converted = <$typ>::try_from(raw);
                debug_assert!(converted.is_ok());
                Some(match converted {
                    Ok(value) => value,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                })
            }

            #[inline]
            pub fn pop_first(&self) -> Option<$typ>
            where
                $typ: TryFrom<u8>,
            {
                let raw = self.0.pop_first()?;
                let converted = <$typ>::try_from(raw);
                debug_assert!(converted.is_ok());
                Some(match converted {
                    Ok(value) => value,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                })
            }

            #[inline]
            pub fn pop_last(&self) -> Option<$typ>
            where
                $typ: TryFrom<u8>,
            {
                let raw = self.0.pop_last()?;
                let converted = <$typ>::try_from(raw);
                debug_assert!(converted.is_ok());
                Some(match converted {
                    Ok(value) => value,
                    Err(_) => unsafe { core::hint::unreachable_unchecked() },
                })
            }

            #[inline]
            pub fn is_subset(&self, other: &Self) -> bool {
                self.0.is_subset(&other.0)
            }

            #[inline]
            pub fn is_superset(&self, other: &Self) -> bool {
                self.0.is_superset(&other.0)
            }

            #[inline]
            pub fn is_disjoint(&self, other: &Self) -> bool {
                self.0.is_disjoint(&other.0)
            }

            #[inline]
            pub fn retain(&self, mut f: impl FnMut($typ) -> bool)
            where
                $typ: TryFrom<u8>,
            {
                self.0.retain(|raw| {
                    let converted = <$typ>::try_from(raw);
                    debug_assert!(converted.is_ok());
                    let value = match converted {
                        Ok(v) => v,
                        Err(_) => unsafe { core::hint::unreachable_unchecked() },
                    };
                    f(value)
                });
            }

            #[inline]
            pub fn iter(&self) -> impl Iterator<Item = $typ>
            where
                $typ: TryFrom<u8>,
            {
                self.0.iter().map(|raw| {
                    let converted = <$typ>::try_from(raw);
                    debug_assert!(converted.is_ok());
                    match converted {
                        Ok(value) => value,
                        Err(_) => unsafe { core::hint::unreachable_unchecked() },
                    }
                })
            }

            #[inline]
            pub fn iter_names(&self) -> impl Iterator<Item = (&'static str, $typ)> + '_
            where
                $typ: $crate::BitFlag,
            {
                <$typ as $crate::BitFlag>::FLAGS.iter().filter_map(move |flag| {
                    let value = *flag.value();
                    if self.contains(&value) {
                        Some((flag.name(), value))
                    } else {
                        None
                    }
                })
            }
        }

        impl From<$set> for $name {
            #[inline]
            fn from(value: $set) -> Self {
                Self::from_plain(value)
            }
        }

        impl From<&$name> for $set {
            #[inline]
            fn from(value: &$name) -> Self {
                <$set>::from_bits_retain(value.bits())
            }
        }

        impl From<$name> for $set {
            #[inline]
            fn from(value: $name) -> Self {
                value.into_plain()
            }
        }

        impl core::fmt::Debug for $name
        where
            $typ: core::fmt::Debug + TryFrom<u8>,
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&format_args!("0x{:x}", self.bits()))
                    .finish()?;
                write!(f, "/* elements: [")?;
                let mut first = true;
                for elem in self.iter() {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "{elem:?}")?;
                    first = false;
                }
                write!(f, "] */")
            }
        }

        impl core::fmt::Binary for $name
        where
            <$atomic as $crate::__private::radium::Radium>::Item: core::fmt::Binary,
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::Binary::fmt(&self.bits(), f)
            }
        }

        impl core::fmt::Octal for $name
        where
            <$atomic as $crate::__private::radium::Radium>::Item: core::fmt::Octal,
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::Octal::fmt(&self.bits(), f)
            }
        }

        impl core::fmt::LowerHex for $name
        where
            <$atomic as $crate::__private::radium::Radium>::Item: core::fmt::LowerHex,
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::LowerHex::fmt(&self.bits(), f)
            }
        }

        impl core::fmt::UpperHex for $name
        where
            <$atomic as $crate::__private::radium::Radium>::Item: core::fmt::UpperHex,
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::UpperHex::fmt(&self.bits(), f)
            }
        }
    };

    // Internal position form implementation
    (@position_impl $vis:vis struct $name:ident($atomic:ty, $set:ty) {
        $($(#[$inner:meta])* const $flag:ident = $value:expr;)*
    }) => {
        $vis struct $name($crate::AtomicBitSet<$atomic, u8>);

        $(
            const _: () = assert!(
                $value < (core::mem::size_of::<$atomic>() * 8) as u8,
                "atomic_bitflagset! position constant exceeds storage width"
            );
            const _: () = assert!(
                $value == <$set>::$flag,
                "atomic_bitflagset! constant must match linked non-atomic set"
            );
        )*

        impl Default for $name {
            #[inline]
            fn default() -> Self {
                Self::new()
            }
        }

        #[allow(dead_code, non_upper_case_globals)]
        impl $name {
            $(
                $(#[$inner])*
                pub const $flag: u8 = $value;
            )*

            const ALL_MASK: <$atomic as $crate::__private::radium::Radium>::Item =
                0 $(| ((1 as <$atomic as $crate::__private::radium::Radium>::Item) << $value))*;

            #[inline]
            fn from_raw(bits: <$atomic as $crate::__private::radium::Radium>::Item) -> Self {
                Self($crate::AtomicBitSet::<$atomic, u8>::from_bits(<$atomic>::new(bits)))
            }

            #[inline]
            pub const fn new() -> Self {
                Self($crate::AtomicBitSet::<$atomic, u8>::new())
            }

            #[inline]
            pub const fn empty() -> Self {
                Self::new()
            }

            #[inline]
            pub fn bits(&self) -> <$atomic as $crate::__private::radium::Radium>::Item {
                self.0.load_store()
            }

            #[inline]
            pub fn from_bits(bits: <$atomic as $crate::__private::radium::Radium>::Item) -> Option<Self> {
                if bits & !Self::ALL_MASK == 0 {
                    Some(Self::from_raw(bits))
                } else {
                    None
                }
            }

            #[inline]
            pub fn from_bits_retain(bits: <$atomic as $crate::__private::radium::Radium>::Item) -> Self {
                Self::from_raw(bits)
            }

            #[inline]
            pub unsafe fn from_bits_unchecked(bits: <$atomic as $crate::__private::radium::Radium>::Item) -> Self {
                Self::from_raw(bits)
            }

            #[inline]
            pub fn from_bits_truncate(bits: <$atomic as $crate::__private::radium::Radium>::Item) -> Self {
                Self::from_raw(bits & Self::ALL_MASK)
            }

            #[inline]
            pub fn from_plain(set: $set) -> Self {
                let bits: <$atomic as $crate::__private::radium::Radium>::Item = set.bits();
                Self::from_raw(bits)
            }

            #[inline]
            pub fn into_plain(self) -> $set {
                <$set>::from_bits_retain(self.bits())
            }

            #[inline]
            pub fn swap_bits(&self, bits: &mut <$atomic as $crate::__private::radium::Radium>::Item) {
                self.0.swap_store(bits);
            }

            #[inline]
            pub fn from_element(pos: u8) -> Self {
                let bits = (core::mem::size_of::<$atomic>() * 8) as u8;
                debug_assert!(pos < bits);
                if pos < bits {
                    Self::from_raw((1 as <$atomic as $crate::__private::radium::Radium>::Item) << pos)
                } else {
                    Self::empty()
                }
            }

            #[inline]
            pub fn from_slice(positions: &[u8]) -> Self {
                let max = (core::mem::size_of::<$atomic>() * 8) as u8;
                let mut raw: <$atomic as $crate::__private::radium::Radium>::Item = 0;
                for &pos in positions {
                    debug_assert!(pos < max);
                    if pos < max {
                        raw |= (1 as <$atomic as $crate::__private::radium::Radium>::Item) << pos;
                    }
                }
                Self::from_raw(raw)
            }

            #[inline]
            pub fn all() -> Self {
                Self::from_raw(Self::ALL_MASK)
            }

            #[inline]
            pub fn is_all(&self) -> bool {
                self.bits() & Self::ALL_MASK == Self::ALL_MASK
            }

            #[inline]
            pub fn complement(&self) -> Self {
                Self::from_raw(Self::ALL_MASK & !self.bits())
            }

            #[inline]
            pub fn len(&self) -> usize {
                self.0.len()
            }

            #[inline]
            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }

            #[inline]
            pub fn contains(&self, pos: &u8) -> bool {
                self.0.contains(pos)
            }

            #[inline]
            pub fn set(&self, pos: u8, value: bool) {
                self.0.set(pos, value);
            }

            #[inline]
            pub fn insert(&self, pos: u8) -> bool {
                self.0.insert(pos)
            }

            #[inline]
            pub fn remove(&self, pos: u8) -> bool {
                self.0.remove(pos)
            }

            #[inline]
            pub fn toggle(&self, pos: u8) {
                self.0.toggle(pos);
            }

            #[inline]
            pub fn clear(&self) {
                self.0.clear();
            }

            #[inline]
            pub fn first(&self) -> Option<u8> {
                self.0.first()
            }

            #[inline]
            pub fn last(&self) -> Option<u8> {
                self.0.last()
            }

            #[inline]
            pub fn pop_first(&self) -> Option<u8> {
                self.0.pop_first()
            }

            #[inline]
            pub fn pop_last(&self) -> Option<u8> {
                self.0.pop_last()
            }

            #[inline]
            pub fn is_subset(&self, other: &Self) -> bool {
                self.0.is_subset(&other.0)
            }

            #[inline]
            pub fn is_superset(&self, other: &Self) -> bool {
                self.0.is_superset(&other.0)
            }

            #[inline]
            pub fn is_disjoint(&self, other: &Self) -> bool {
                self.0.is_disjoint(&other.0)
            }

            #[inline]
            pub fn retain(&self, f: impl FnMut(u8) -> bool) {
                self.0.retain(f);
            }

            #[inline]
            pub fn iter(&self) -> impl Iterator<Item = u8> {
                self.0.iter()
            }

            pub fn iter_names(&self) -> impl Iterator<Item = (&'static str, u8)> {
                let bits = self.bits();
                [
                    $((stringify!($flag), $name::$flag, (1 as <$atomic as $crate::__private::radium::Radium>::Item) << $name::$flag)),*
                ]
                .into_iter()
                .filter(move |(_, _, mask)| bits & *mask != 0)
                .map(|(name, pos, _)| (name, pos))
            }
        }

        impl From<$set> for $name {
            #[inline]
            fn from(value: $set) -> Self {
                Self::from_plain(value)
            }
        }

        impl From<&$name> for $set {
            #[inline]
            fn from(value: &$name) -> Self {
                <$set>::from_bits_retain(value.bits())
            }
        }

        impl From<$name> for $set {
            #[inline]
            fn from(value: $name) -> Self {
                value.into_plain()
            }
        }

        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                let mut remaining = self.bits();
                write!(f, "{}(", stringify!($name))?;
                let mut first = true;
                $(
                    {
                        let mask: <$atomic as $crate::__private::radium::Radium>::Item =
                            (1 as <$atomic as $crate::__private::radium::Radium>::Item) << $name::$flag;
                        if remaining & mask != 0 {
                            if !first { write!(f, " | ")?; }
                            write!(f, "{}", stringify!($flag))?;
                            remaining &= !mask;
                            first = false;
                        }
                    }
                )*
                if remaining != 0 {
                    if !first { write!(f, " | ")?; }
                    write!(f, "0x{:x}", remaining)?;
                } else if first {
                    write!(f, "empty")?;
                }
                write!(f, ")")
            }
        }

        impl core::fmt::Binary for $name
        where
            <$atomic as $crate::__private::radium::Radium>::Item: core::fmt::Binary,
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::Binary::fmt(&self.bits(), f)
            }
        }

        impl core::fmt::Octal for $name
        where
            <$atomic as $crate::__private::radium::Radium>::Item: core::fmt::Octal,
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::Octal::fmt(&self.bits(), f)
            }
        }

        impl core::fmt::LowerHex for $name
        where
            <$atomic as $crate::__private::radium::Radium>::Item: core::fmt::LowerHex,
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::LowerHex::fmt(&self.bits(), f)
            }
        }

        impl core::fmt::UpperHex for $name
        where
            <$atomic as $crate::__private::radium::Radium>::Item: core::fmt::UpperHex,
        {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::UpperHex::fmt(&self.bits(), f)
            }
        }
    };
}

#[cfg(test)]
mod bitflag_tests {
    use super::BitFlag;

    crate::bitflag! {
        #[derive(Debug)]
        #[repr(u8)]
        enum Color {
            Red = 0,
            Green = 1,
            Blue = 2,
        }
    }

    crate::bitflagset!(struct ColorSet(u8) : Color);

    #[test]
    fn flags_list() {
        assert_eq!(Color::FLAGS.len(), 3);
        assert_eq!(Color::FLAGS[0].name(), "Red");
        assert_eq!(Color::FLAGS[1].name(), "Green");
        assert_eq!(Color::FLAGS[2].name(), "Blue");
        assert_eq!(*Color::FLAGS[0].value(), Color::Red);
    }

    #[test]
    fn as_u8_and_as_usize() {
        assert_eq!(Color::Red.as_u8(), 0);
        assert_eq!(Color::Green.as_u8(), 1);
        assert_eq!(Color::Blue.as_usize(), 2);
    }

    #[test]
    fn mask() {
        assert_eq!(Color::Red.mask(), 0b001u8);
        assert_eq!(Color::Green.mask(), 0b010u8);
        assert_eq!(Color::Blue.mask(), 0b100u8);
    }

    #[test]
    fn mask_bitwise() {
        let store: u8 = 0b101;
        assert!(store & Color::Red.mask() != 0);
        assert!(store & Color::Green.mask() == 0);
        assert!(store & Color::Blue.mask() != 0);
    }

    #[test]
    fn try_from_u8() {
        assert_eq!(Color::try_from(0), Ok(Color::Red));
        assert_eq!(Color::try_from(1), Ok(Color::Green));
        assert_eq!(Color::try_from(2), Ok(Color::Blue));
        assert!(Color::try_from(3).is_err());
        assert!(Color::try_from(255).is_err());
    }

    #[test]
    fn from_color_for_u8() {
        assert_eq!(u8::from(Color::Red), 0);
        assert_eq!(u8::from(Color::Green), 1);
        assert_eq!(u8::from(Color::Blue), 2);
    }

    #[test]
    fn with_bitflagset() {
        let mut set = ColorSet::from_slice(&[Color::Red, Color::Blue]);
        assert!(set.contains(&Color::Red));
        assert!(!set.contains(&Color::Green));
        set.insert(Color::Green);
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn bitflagset_all_complement() {
        let all = ColorSet::all();
        assert_eq!(all.len(), 3);
        assert!(all.is_all());
        assert!(all.contains(&Color::Red));
        assert!(all.contains(&Color::Green));
        assert!(all.contains(&Color::Blue));

        let r = ColorSet::from_element(Color::Red);
        let comp = r.complement();
        assert!(!comp.contains(&Color::Red));
        assert!(comp.contains(&Color::Green));
        assert!(comp.contains(&Color::Blue));
    }

    #[test]
    fn bitflagset_not_truncated() {
        let r = ColorSet::from_element(Color::Red);
        let not_r = !r;
        // Not should be truncated: only defined bits, not all 8 bits
        assert_eq!(not_r.len(), 2);
        assert!(!not_r.contains(&Color::Red));
        assert!(not_r.contains(&Color::Green));
        assert!(not_r.contains(&Color::Blue));
    }

    #[test]
    fn bitflagset_toggle() {
        let mut set = ColorSet::from_element(Color::Red);
        set.toggle(Color::Green);
        assert!(set.contains(&Color::Green));
        set.toggle(Color::Red);
        assert!(!set.contains(&Color::Red));
    }

    #[test]
    fn bitflagset_format_traits() {
        extern crate alloc;
        use alloc::format;
        let set = ColorSet::from_bits_retain(0b101);
        assert_eq!(format!("{set:b}"), "101");
        assert_eq!(format!("{set:o}"), "5");
        assert_eq!(format!("{set:x}"), "5");
        assert_eq!(format!("{set:X}"), "5");
    }

    #[test]
    fn bitflagset_iter_names() {
        extern crate alloc;
        use alloc::vec::Vec;
        let set = ColorSet::from_slice(&[Color::Red, Color::Blue]);
        let names: Vec<_> = set.iter_names().collect();
        assert_eq!(names, [("Red", Color::Red), ("Blue", Color::Blue)]);
    }

    #[test]
    fn bitflagset_retain() {
        let mut set = ColorSet::from_slice(&[Color::Red, Color::Green, Color::Blue]);
        set.retain(|c| c != Color::Green);
        assert!(set.contains(&Color::Red));
        assert!(!set.contains(&Color::Green));
        assert!(set.contains(&Color::Blue));
        assert_eq!(set.len(), 2);

        // retain all — no change
        let mut all = ColorSet::all();
        all.retain(|_| true);
        assert_eq!(all.len(), 3);

        // retain none — empty
        let mut all2 = ColorSet::all();
        all2.retain(|_| false);
        assert!(all2.is_empty());
    }

    #[test]
    fn bitflagset_api_coverage() {
        let mut set = ColorSet::from_bits(0b111).unwrap();
        assert!(set.is_all());
        set.set(Color::Red, false);
        assert!(!set.contains(&Color::Red));
        set.toggle(Color::Red);
        assert!(set.contains(&Color::Red));
        set.clear();
        assert!(set.is_empty());
        assert_eq!(ColorSet::from_bits_truncate(0xFF), ColorSet::all());
        let unchecked = unsafe { ColorSet::from_bits_unchecked(0b001) };
        assert!(unchecked.contains(&Color::Red));
    }

    #[test]
    fn auto_discriminants() {
        crate::bitflag! {
            #[derive(Debug)]
            #[repr(u8)]
            enum Shape {
                Circle,
                Square,
                Triangle,
            }
        }
        assert_eq!(Shape::Circle as u8, 0);
        assert_eq!(Shape::Square as u8, 1);
        assert_eq!(Shape::Triangle as u8, 2);
        assert_eq!(Shape::FLAGS.len(), 3);
        assert_eq!(Shape::FLAGS[0].name(), "Circle");

        crate::bitflagset!(struct ShapeSet(u8) : Shape);
        let mut set = ShapeSet::from_element(Shape::Circle);
        set.insert(Shape::Triangle);
        assert!(set.contains(&Shape::Circle));
        assert!(!set.contains(&Shape::Square));
        assert!(set.contains(&Shape::Triangle));
        assert_eq!(set.len(), 2);
    }
}

#[cfg(all(test, feature = "bitflags"))]
mod bitflags_enum_tests {
    use bitflags::Flags;

    crate::bitflag! {
        #[derive(Debug)]
        #[repr(u8)]
        enum Color {
            Red = 0,
            Green = 1,
            Blue = 2,
        }
    }

    crate::bitflagset!(struct ColorSet(u8) : Color);

    #[test]
    fn flags_bits_roundtrip() {
        let set = ColorSet::from_slice(&[Color::Red, Color::Blue]);
        let bits = Flags::bits(&set);
        assert_eq!(<ColorSet as Flags>::from_bits_retain(bits), set);
    }

    #[test]
    fn flags_from_bits_validates() {
        let all_bits = (1u8 << 0) | (1u8 << 1) | (1u8 << 2);
        assert!(ColorSet::from_bits(all_bits).is_some());
        assert!(ColorSet::from_bits(1u8 << 7).is_none());
    }

    #[test]
    fn flags_all_empty() {
        let all = ColorSet::all();
        assert!(all.contains(&Color::Red));
        assert!(all.contains(&Color::Green));
        assert!(all.contains(&Color::Blue));
        assert_eq!(all.len(), 3);

        let empty = <ColorSet as Flags>::empty();
        assert!(empty.is_empty());
    }

    #[test]
    fn flags_complement() {
        let r = ColorSet::from_element(Color::Red);
        let comp = Flags::complement(r);
        assert!(!comp.contains(&Color::Red));
        assert!(comp.contains(&Color::Green));
        assert!(comp.contains(&Color::Blue));
    }

    #[test]
    fn flags_insert_remove() {
        let mut set = <ColorSet as Flags>::empty();
        Flags::insert(&mut set, ColorSet::from_element(Color::Green));
        assert!(set.contains(&Color::Green));
        Flags::remove(&mut set, ColorSet::from_element(Color::Green));
        assert!(!set.contains(&Color::Green));
    }

    #[test]
    fn inherent_api_coverage() {
        let mut set = ColorSet::from_bits(0b111).unwrap();
        assert!(set.is_all());
        set.set(Color::Blue, false);
        set.toggle(Color::Blue);
        set.clear();
        assert!(set.is_empty());
        assert_eq!(ColorSet::from_bits_truncate(0xFF), ColorSet::all());
        let unchecked = unsafe { ColorSet::from_bits_unchecked(0b001) };
        assert!(unchecked.contains(&Color::Red));
        let _ = ColorSet::from_slice(&[Color::Red, Color::Blue])
            .iter_names()
            .count();
    }
}

#[cfg(test)]
mod bitflags_mode_tests {
    extern crate alloc;
    use alloc::{format, vec, vec::Vec};

    crate::bitflagset! {
        struct Perms(u8) {
            const READ = 0;
            const WRITE = 1;
            const EXEC = 2;
        }
    }

    // Composite constant defined outside the macro
    impl Perms {
        const RW: Self = Self::from_slice(&[Self::READ, Self::WRITE]);
    }

    #[test]
    fn basic_constants() {
        assert_eq!(Perms::READ, 0u8);
        assert_eq!(Perms::WRITE, 1u8);
        assert_eq!(Perms::EXEC, 2u8);
        assert_eq!(Perms::RW.bits(), 0b0011);
    }

    #[test]
    fn all_and_empty() {
        let all = Perms::all();
        assert!(all.contains(&Perms::READ));
        assert!(all.contains(&Perms::WRITE));
        assert!(all.contains(&Perms::EXEC));
        assert!(all.is_all());
        assert_eq!(all.bits(), 0b0111);

        let empty = Perms::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert!(!empty.is_all());
    }

    #[test]
    fn contains_insert_remove() {
        let mut p = Perms::empty();

        assert!(p.insert(Perms::READ));
        assert!(p.contains(&Perms::READ));
        assert!(!p.insert(Perms::READ)); // already present

        assert!(p.insert(Perms::WRITE));
        assert_eq!(p, Perms::RW);

        assert!(p.remove(Perms::WRITE));
        assert!(!p.contains(&Perms::WRITE));
        assert!(!p.remove(Perms::WRITE)); // already absent
        assert!(p.contains(&Perms::READ));

        p.set(Perms::EXEC, true);
        assert!(p.contains(&Perms::EXEC));
        p.set(Perms::EXEC, false);
        assert!(!p.contains(&Perms::EXEC));

        p.clear();
        assert!(p.is_empty());
    }

    #[test]
    fn from_element_and_from_slice() {
        let r = Perms::from_element(Perms::READ);
        assert!(r.contains(&Perms::READ));
        assert!(!r.contains(&Perms::WRITE));

        assert!(Perms::RW.contains(&Perms::READ));
        assert!(Perms::RW.contains(&Perms::WRITE));
        assert!(!Perms::RW.contains(&Perms::EXEC));
    }

    #[test]
    fn set_algebra() {
        let re = Perms::from_slice(&[Perms::READ, Perms::EXEC]);

        assert!(Perms::from_element(Perms::READ).is_subset(&Perms::RW));
        assert!(!Perms::from_element(Perms::EXEC).is_subset(&Perms::RW));
        assert!(Perms::RW.is_superset(&Perms::from_element(Perms::READ)));
        assert!(!Perms::RW.is_disjoint(&re));
        assert!(Perms::from_element(Perms::WRITE).is_disjoint(&Perms::from_element(Perms::EXEC)));
    }

    #[test]
    fn complement() {
        let r = Perms::from_element(Perms::READ);
        let comp = r.complement();
        assert!(!comp.contains(&Perms::READ));
        assert!(comp.contains(&Perms::WRITE));
        assert!(comp.contains(&Perms::EXEC));

        let rw_comp = Perms::RW.complement();
        assert_eq!(rw_comp, Perms::from_element(Perms::EXEC));
    }

    #[test]
    fn operators() {
        let a = Perms::RW;
        let b = Perms::from_slice(&[Perms::WRITE, Perms::EXEC]);

        assert_eq!((a | b).bits(), 0b0111);
        assert_eq!((a & b).bits(), 0b0010);
        assert_eq!((a ^ b).bits(), 0b0101);
        assert_eq!((a - b).bits(), 0b0001);

        let mut c = Perms::from_element(Perms::READ);
        c |= Perms::from_element(Perms::WRITE);
        assert_eq!(c, Perms::RW);
        c &= Perms::from_element(Perms::READ);
        assert_eq!(c, Perms::from_element(Perms::READ));
    }

    #[test]
    fn iter_and_collect() {
        let items: Vec<u8> = Perms::RW.iter().collect();
        assert_eq!(items, vec![Perms::READ, Perms::WRITE]);

        let collected: Perms = items.into_iter().collect();
        assert_eq!(collected, Perms::RW);
    }

    #[test]
    fn into_iterator() {
        let flags = Perms::from_slice(&[Perms::READ, Perms::EXEC]);
        let items: Vec<u8> = flags.into_iter().collect();
        assert_eq!(items, vec![Perms::READ, Perms::EXEC]);
    }

    #[test]
    fn extend() {
        let mut p = Perms::from_element(Perms::READ);
        p.extend([Perms::WRITE, Perms::EXEC]);
        assert_eq!(p, Perms::all());
    }

    #[test]
    fn from_bits_variants() {
        assert_eq!(Perms::from_bits(0b0011), Some(Perms::RW));
        assert_eq!(Perms::from_bits(0b0111), Some(Perms::all()));
        assert_eq!(Perms::from_bits(0b1000), None);

        assert_eq!(Perms::from_bits_retain(0xFF).bits(), 0xFF);
        assert_eq!(Perms::from_bits_truncate(0xFF), Perms::all());
    }

    #[test]
    fn debug_format() {
        let empty = Perms::empty();
        assert_eq!(format!("{empty:?}"), "Perms(empty)");

        let r = Perms::from_element(Perms::READ);
        assert_eq!(format!("{r:?}"), "Perms(READ)");

        assert_eq!(format!("{:?}", Perms::RW), "Perms(READ | WRITE)");

        let all = Perms::all();
        assert_eq!(format!("{all:?}"), "Perms(READ | WRITE | EXEC)");

        let unknown = Perms::from_bits_retain(0b1000_0001);
        assert_eq!(format!("{unknown:?}"), "Perms(READ | 0x80)");
    }

    #[test]
    fn len_counting() {
        assert_eq!(Perms::empty().len(), 0);
        assert_eq!(Perms::from_element(Perms::READ).len(), 1);
        assert_eq!(Perms::RW.len(), 2);
        assert_eq!(Perms::all().len(), 3);
    }

    #[test]
    fn toggle() {
        let mut p = Perms::from_element(Perms::READ);
        p.toggle(Perms::WRITE);
        assert!(p.contains(&Perms::WRITE));
        assert!(p.contains(&Perms::READ));
        p.toggle(Perms::READ);
        assert!(!p.contains(&Perms::READ));
        assert!(p.contains(&Perms::WRITE));
    }

    #[test]
    fn not_truncated() {
        let r = Perms::from_element(Perms::READ);
        let not_r = !r;
        // Not should be truncated to defined bits only
        assert_eq!(not_r.len(), 2);
        assert!(!not_r.contains(&Perms::READ));
        assert!(not_r.contains(&Perms::WRITE));
        assert!(not_r.contains(&Perms::EXEC));
        assert_eq!(not_r.bits(), 0b110);
    }

    #[test]
    fn iter_names() {
        let rw = Perms::RW;
        let names: Vec<(&str, u8)> = rw.iter_names().collect();
        assert_eq!(names, [("READ", Perms::READ), ("WRITE", Perms::WRITE)]);

        let empty_names: Vec<(&str, u8)> = Perms::empty().iter_names().collect();
        assert!(empty_names.is_empty());
    }

    #[test]
    fn from_iter_self() {
        let sets = [
            Perms::from_element(Perms::READ),
            Perms::from_element(Perms::EXEC),
        ];
        let merged: Perms = sets.into_iter().collect();
        assert!(merged.contains(&Perms::READ));
        assert!(merged.contains(&Perms::EXEC));
        assert!(!merged.contains(&Perms::WRITE));
    }

    #[test]
    fn extend_self() {
        let mut p = Perms::from_element(Perms::READ);
        p.extend([
            Perms::from_element(Perms::WRITE),
            Perms::from_element(Perms::EXEC),
        ]);
        assert_eq!(p, Perms::all());
    }

    #[test]
    fn format_traits() {
        let p = Perms::RW;
        assert_eq!(format!("{p:b}"), "11");
        assert_eq!(format!("{p:o}"), "3");
        assert_eq!(format!("{p:x}"), "3");
        assert_eq!(format!("{p:X}"), "3");
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn out_of_range_position_is_ignored_in_release() {
        let mut p = Perms::empty();
        assert_eq!(Perms::from_element(8), Perms::empty());
        assert_eq!(
            Perms::from_slice(&[Perms::READ, 8]),
            Perms::from_element(Perms::READ)
        );
        assert!(!p.contains(&8));
        assert!(!p.insert(8));
        assert!(!p.remove(8));
        p.set(8, true);
        p.toggle(8);
        assert!(p.is_empty());
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn out_of_range_position_panics_in_debug() {
        let p = Perms::empty();
        let _ = p.contains(&8);
    }

    #[test]
    fn retain() {
        let mut p = Perms::from_slice(&[Perms::READ, Perms::WRITE, Perms::EXEC]);
        p.retain(|pos| pos != Perms::WRITE);
        assert!(p.contains(&Perms::READ));
        assert!(!p.contains(&Perms::WRITE));
        assert!(p.contains(&Perms::EXEC));
        assert_eq!(p.len(), 2);

        // retain all — no change
        let mut all = Perms::all();
        all.retain(|_| true);
        assert_eq!(all.len(), 3);

        // retain none — empty
        let mut all2 = Perms::all();
        all2.retain(|_| false);
        assert!(all2.is_empty());
    }
}

#[cfg(all(test, feature = "bitflags"))]
mod bitflags_mode_interop_tests {
    use bitflags::Flags;

    crate::bitflagset! {
        struct Perms(u8) {
            const READ = 0;
            const WRITE = 1;
            const EXEC = 2;
        }
    }

    #[test]
    fn flags_trait_basics() {
        let all = Perms::all();
        let bits = Flags::bits(&all);
        assert_eq!(Perms::from_bits_retain(bits), all);
    }

    #[test]
    fn flags_from_bits() {
        assert!(Perms::from_bits(0b0111).is_some());
        assert!(Perms::from_bits(0b1000).is_none());
    }

    #[test]
    fn flags_all_empty() {
        let all = <Perms as Flags>::all();
        assert_eq!(all.len(), 3);

        let empty = <Perms as Flags>::empty();
        assert!(empty.is_empty());
    }

    #[test]
    fn flags_complement() {
        let r = Perms::from_element(Perms::READ);
        let comp = Flags::complement(r);
        assert!(!comp.contains(&Perms::READ));
        assert!(comp.contains(&Perms::WRITE));
        assert!(comp.contains(&Perms::EXEC));
    }

    #[test]
    fn flags_insert_remove() {
        let mut set = <Perms as Flags>::empty();
        Flags::insert(&mut set, Perms::from_element(Perms::WRITE));
        assert!(set.contains(&Perms::WRITE));
        Flags::remove(&mut set, Perms::from_element(Perms::WRITE));
        assert!(!set.contains(&Perms::WRITE));
    }
}

#[cfg(test)]
mod atomic_bitflagset_tests {
    extern crate alloc;
    use alloc::format;
    use core::sync::atomic::AtomicU8;

    crate::bitflag! {
        #[repr(u8)]
        #[derive(Debug)]
        enum Color {
            Red = 0,
            Green = 1,
            Blue = 2,
        }
    }

    crate::bitflagset!(struct ColorSet(u8) : Color);
    crate::atomic_bitflagset!(struct AtomicColorSet(AtomicU8) on ColorSet);

    crate::bitflagset! {
        struct Perms(u8) {
            const READ = 0;
            const WRITE = 1;
            const EXEC = 2;
        }
    }

    crate::atomic_bitflagset! {
        struct AtomicPerms(AtomicU8) on Perms {
            const READ = 0;
            const WRITE = 1;
            const EXEC = 2;
        }
    }

    #[test]
    fn enum_form_set_interop() {
        let plain = ColorSet::from_slice(&[Color::Red, Color::Blue]);
        let atomic = AtomicColorSet::from_plain(plain);
        assert_eq!(ColorSet::from(&atomic), plain);

        let converted_atomic: AtomicColorSet = plain.into();
        let converted_plain: ColorSet = (&converted_atomic).into();
        assert_eq!(converted_plain, plain);
    }

    #[test]
    fn plain_color_set_api_coverage() {
        let mut set = ColorSet::from_bits(0b111).unwrap();
        assert!(set.is_all());
        set.set(Color::Red, false);
        set.toggle(Color::Red);
        set.clear();
        assert!(set.is_empty());
        assert_eq!(ColorSet::from_bits_truncate(0xFF), ColorSet::all());
        let unchecked = unsafe { ColorSet::from_bits_unchecked(0b001) };
        assert!(unchecked.contains(&Color::Red));
        let _ = ColorSet::from_slice(&[Color::Red]).iter_names().count();
    }

    #[test]
    fn atomic_color_set_api_coverage() {
        let empty = AtomicColorSet::empty();
        assert_eq!(empty.bits(), 0);
        let _ = AtomicColorSet::from_slice(&[Color::Red, Color::Blue]);

        let from_plain = AtomicColorSet::from_plain(ColorSet::from_element(Color::Green));
        let _owned_plain = from_plain.into_plain();

        let unchecked = unsafe { AtomicColorSet::from_bits_unchecked(0b001) };
        assert!(unchecked.contains(&Color::Red));
        assert!(AtomicColorSet::from_bits(0b001).is_some());
        assert!(AtomicColorSet::from_bits(1u8 << 7).is_none());
        assert_eq!(AtomicColorSet::from_bits_truncate(0xFF).bits(), AtomicColorSet::all().bits());

        let all = AtomicColorSet::all();
        assert!(all.is_all());
        let _ = all.complement();

        let a = AtomicColorSet::new();
        assert!(a.is_empty());
        assert_eq!(a.len(), 0);

        a.set(Color::Red, true);
        assert!(a.contains(&Color::Red));
        let _ = a.insert(Color::Green);
        let _ = a.remove(Color::Green);
        a.toggle(Color::Blue);

        let mut raw = ColorSet::from_element(Color::Red).bits();
        a.swap_bits(&mut raw);

        let _ = a.first();
        let _ = a.last();
        let _ = a.pop_first();
        let _ = a.pop_last();

        let b = AtomicColorSet::from_element(Color::Red);
        let _ = a.is_subset(&b);
        let _ = a.is_superset(&b);
        let _ = a.is_disjoint(&b);

        a.retain(|_| true);
        let _ = a.iter().count();
        let _ = a.iter_names().count();

        let _ = format!("{a:b}");
        let _ = format!("{a:o}");
        let _ = format!("{a:x}");
        let _ = format!("{a:X}");
        a.clear();
    }

    #[test]
    fn enum_form_swap_bits() {
        let atomic = AtomicColorSet::new();
        atomic.insert(Color::Green);
        assert_eq!(ColorSet::from(&atomic), ColorSet::from_element(Color::Green));

        let mut raw = ColorSet::from_element(Color::Red).bits();
        atomic.swap_bits(&mut raw);
        assert_eq!(raw, ColorSet::from_element(Color::Green).bits());
        assert_eq!(ColorSet::from(&atomic), ColorSet::from_element(Color::Red));

        atomic.insert(Color::Blue);
        assert_eq!(
            ColorSet::from(&atomic),
            ColorSet::from_slice(&[Color::Red, Color::Blue])
        );
    }

    #[test]
    fn position_form_set_interop() {
        let plain = Perms::from_slice(&[Perms::READ, Perms::EXEC]);
        let atomic = AtomicPerms::from_plain(plain);
        assert_eq!(Perms::from(&atomic), plain);

        let mut raw = Perms::from_element(Perms::WRITE).bits();
        atomic.swap_bits(&mut raw);
        assert_eq!(raw, plain.bits());
        assert_eq!(Perms::from(&atomic), Perms::from_element(Perms::WRITE));

        atomic.insert(Perms::EXEC);
        assert_eq!(
            Perms::from(&atomic),
            Perms::from_slice(&[Perms::WRITE, Perms::EXEC])
        );
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn position_form_oob_ignored_in_release() {
        let atomic = AtomicPerms::from_element(8);
        assert_eq!(Perms::from(&atomic), Perms::empty());

        let atomic = AtomicPerms::from_slice(&[Perms::READ, 8]);
        assert_eq!(Perms::from(&atomic), Perms::from_element(Perms::READ));
    }
}
