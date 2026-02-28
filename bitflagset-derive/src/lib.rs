use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derive the `BitFlag` trait for a `#[repr(u8)]` enum.
///
/// This generates:
/// - `From<EnumName> for u8`
/// - `TryFrom<u8> for EnumName`
/// - `impl BitFlag for EnumName`
///
/// The enum must have `#[repr(u8)]` and all variants must be unit variants.
///
/// ```ignore
/// #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, BitFlag)]
/// #[repr(u8)]
/// enum Color {
///     Red,
///     Green,
///     Blue,
/// }
/// ```
#[proc_macro_derive(BitFlag)]
pub fn derive_bitflag(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_bitflag(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn impl_bitflag(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;

    let data = match &input.data {
        syn::Data::Enum(data) => data,
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "BitFlag can only be derived for enums",
            ));
        }
    };

    let variants: Vec<&syn::Ident> = data
        .variants
        .iter()
        .map(|v| {
            if !matches!(v.fields, syn::Fields::Unit) {
                return Err(syn::Error::new_spanned(
                    &v.ident,
                    "BitFlag variants must be unit variants",
                ));
            }
            Ok(&v.ident)
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let variant_names: Vec<String> = variants.iter().map(|v| v.to_string()).collect();

    let flags_entries = variants.iter().zip(variant_names.iter()).map(|(v, s)| {
        quote! { ::bitflagset::Flag::new(#s, #name::#v) }
    });

    let try_from_arms = variants.iter().map(|v| {
        quote! { x if x == #name::#v as u8 => Ok(#name::#v) }
    });

    let max_value_arms = variants.iter().map(|v| {
        quote! {
            let value = #name::#v as u8;
            if value > max {
                max = value;
            }
        }
    });

    Ok(quote! {
        const _: () = assert!(
            core::mem::size_of::<#name>() == core::mem::size_of::<u8>(),
            "BitFlag enum must use #[repr(u8)]"
        );

        impl From<#name> for u8 {
            #[inline]
            fn from(v: #name) -> u8 { v as u8 }
        }

        impl TryFrom<u8> for #name {
            type Error = ();
            fn try_from(v: u8) -> Result<Self, ()> {
                match v {
                    #(#try_from_arms,)*
                    _ => Err(()),
                }
            }
        }

        impl ::bitflagset::BitFlag for #name {
            type Mask = u8;
            const FLAGS: &'static [::bitflagset::Flag<Self>] = &[
                #(#flags_entries),*
            ];
            const MAX_VALUE: u8 = {
                let mut max: u8 = 0;
                #(#max_value_arms)*
                max
            };
        }
    })
}
