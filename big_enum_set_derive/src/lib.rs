#![recursion_limit="256"]
#![cfg_attr(feature = "nightly", feature(proc_macro_diagnostic))]

extern crate proc_macro;

use darling::*;
use proc_macro::TokenStream;
use proc_macro2::{TokenStream as SynTokenStream, Literal};
use syn::*;
use syn::export::Span;
use syn::spanned::Spanned;
use quote::*;

#[cfg(feature = "nightly")]
fn error(span: Span, data: &str) -> TokenStream {
    span.unstable().error(data).emit();
    TokenStream::new()
}

#[cfg(not(feature = "nightly"))]
fn error(_: Span, data: &str) -> TokenStream {
    panic!("{}", data)
}

fn enum_set_type_impl(
    name: &Ident, all_variants: u128, repr: Ident, attrs: EnumsetAttrs,
) -> SynTokenStream {
    let typed_big_enum_set = quote!(::big_enum_set::BigEnumSet<#name>);
    let core = quote!(::big_enum_set::internal::core_export);
    #[cfg(feature = "serde")]
    let serde = quote!(::big_enum_set::internal::serde);

    // proc_macro2 does not support creating u128 literals.
    let all_variants = Literal::u128_unsuffixed(all_variants);

    let ops = if attrs.no_ops {
        quote! {}
    } else {
        quote! {
            impl <O : Into<#typed_big_enum_set>> #core::ops::Sub<O> for #name {
                type Output = #typed_big_enum_set;
                fn sub(self, other: O) -> Self::Output {
                    ::big_enum_set::BigEnumSet::only(self) - other.into()
                }
            }
            impl <O : Into<#typed_big_enum_set>> #core::ops::BitAnd<O> for #name {
                type Output = #typed_big_enum_set;
                fn bitand(self, other: O) -> Self::Output {
                    ::big_enum_set::BigEnumSet::only(self) & other.into()
                }
            }
            impl <O : Into<#typed_big_enum_set>> #core::ops::BitOr<O> for #name {
                type Output = #typed_big_enum_set;
                fn bitor(self, other: O) -> Self::Output {
                    ::big_enum_set::BigEnumSet::only(self) | other.into()
                }
            }
            impl <O : Into<#typed_big_enum_set>> #core::ops::BitXor<O> for #name {
                type Output = #typed_big_enum_set;
                fn bitxor(self, other: O) -> Self::Output {
                    ::big_enum_set::BigEnumSet::only(self) ^ other.into()
                }
            }
            impl #core::ops::Not for #name {
                type Output = #typed_big_enum_set;
                fn not(self) -> Self::Output {
                    !::big_enum_set::BigEnumSet::only(self)
                }
            }
            impl #core::cmp::PartialEq<#typed_big_enum_set> for #name {
                fn eq(&self, other: &#typed_big_enum_set) -> bool {
                    ::big_enum_set::BigEnumSet::only(*self) == *other
                }
            }
        }
    };

    #[cfg(feature = "serde")]
    let serde_ops = if attrs.serialize_as_list {
        let expecting_str = format!("a list of {}", name);
        quote! {
            fn serialize<S: #serde::Serializer>(
                set: ::big_enum_set::BigEnumSet<#name>, ser: S,
            ) -> #core::result::Result<S::Ok, S::Error> {
                use #serde::ser::SerializeSeq;
                let mut seq = ser.serialize_seq(#core::prelude::v1::Some(set.len()))?;
                for bit in set {
                    seq.serialize_element(&bit)?;
                }
                seq.end()
            }
            fn deserialize<'de, D: #serde::Deserializer<'de>>(
                de: D,
            ) -> #core::result::Result<::big_enum_set::BigEnumSet<#name>, D::Error> {
                struct Visitor;
                impl <'de> #serde::de::Visitor<'de> for Visitor {
                    type Value = ::big_enum_set::BigEnumSet<#name>;
                    fn expecting(
                        &self, formatter: &mut #core::fmt::Formatter,
                    ) -> #core::fmt::Result {
                        write!(formatter, #expecting_str)
                    }
                    fn visit_seq<A>(
                        mut self, mut seq: A,
                    ) -> Result<Self::Value, A::Error> where A: #serde::de::SeqAccess<'de> {
                        let mut accum = ::big_enum_set::BigEnumSet::<#name>::new();
                        while let #core::prelude::v1::Some(val) = seq.next_element::<#name>()? {
                            accum |= val;
                        }
                        #core::prelude::v1::Ok(accum)
                    }
                }
                de.deserialize_seq(Visitor)
            }
        }
    } else {
        let serialize_repr = attrs.serialize_repr.as_ref()
            .map(|x| Ident::new(&x, Span::call_site()))
            .unwrap_or(repr.clone());
        let check_unknown = if attrs.serialize_deny_unknown {
            quote! {
                if value & !#all_variants != 0 {
                    use #serde::de::Error;
                    let unexpected = #serde::de::Unexpected::Unsigned(value as u64);
                    return #core::prelude::v1::Err(
                        D::Error::custom("big_enum_set contains unknown bits")
                    )
                }
            }
        } else {
            quote! { }
        };
        quote! {
            fn serialize<S: #serde::Serializer>(
                set: ::big_enum_set::BigEnumSet<#name>, ser: S,
            ) -> #core::result::Result<S::Ok, S::Error> {
                use #serde::Serialize;
                #serialize_repr::serialize(&(set.__big_enum_set_underlying as #serialize_repr), ser)
            }
            fn deserialize<'de, D: #serde::Deserializer<'de>>(
                de: D,
            ) -> #core::result::Result<::big_enum_set::BigEnumSet<#name>, D::Error> {
                use #serde::Deserialize;
                let value = #serialize_repr::deserialize(de)?;
                #check_unknown
                #core::prelude::v1::Ok(::big_enum_set::BigEnumSet {
                    __big_enum_set_underlying: (value & #all_variants) as #repr,
                })
            }
        }
    };

    #[cfg(not(feature = "serde"))]
    let serde_ops = quote! { };

    quote! {
        unsafe impl ::big_enum_set::internal::EnumSetTypePrivate for #name {
            type Repr = #repr;
            const ALL_BITS: Self::Repr = #all_variants;

            fn enum_into_u8(self) -> u8 {
                self as u8
            }
            unsafe fn enum_from_u8(val: u8) -> Self {
                #core::mem::transmute(val)
            }

            #serde_ops
        }

        unsafe impl ::big_enum_set::BigEnumSetType for #name { }

        impl #core::cmp::PartialEq for #name {
            fn eq(&self, other: &Self) -> bool {
                (*self as u8) == (*other as u8)
            }
        }
        impl #core::cmp::Eq for #name { }
        impl #core::clone::Clone for #name {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl #core::marker::Copy for #name { }

        #ops
    }
}

#[derive(FromDeriveInput, Default)]
#[darling(attributes(big_enum_set), default)]
struct EnumsetAttrs {
    no_ops: bool,
    serialize_as_list: bool,
    serialize_deny_unknown: bool,
    #[darling(default)]
    serialize_repr: Option<String>,
}

#[proc_macro_derive(BigEnumSetType, attributes(big_enum_set))]
pub fn derive_enum_set_type(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);
    if let Data::Enum(data) = &input.data {
        if !input.generics.params.is_empty() {
            error(input.generics.span(),
                  "`#[derive(BigEnumSetType)]` cannot be used on enums with type parameters.")
        } else {
            let mut all_variants = 0u128;
            let mut max_variant = 0;
            let mut current_variant = 0;
            let mut has_manual_discriminant = false;

            for variant in &data.variants {
                if let Fields::Unit = variant.fields {
                    if let Some((_, expr)) = &variant.discriminant {
                        if let Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) = expr {
                            current_variant = i.value();
                            has_manual_discriminant = true;
                        } else {
                            return error(variant.span(), "Unrecognized discriminant for variant.")
                        }
                    }

                    if current_variant >= 128 {
                        let message = if has_manual_discriminant {
                            "`#[derive(BigEnumSetType)]` only supports enum discriminants up to 127."
                        } else {
                            "`#[derive(BigEnumSetType)]` only supports enums up to 128 variants."
                        };
                        return error(variant.span(), message)
                    }

                    if all_variants & (1 << current_variant) != 0 {
                        return error(variant.span(),
                                     &format!("Duplicate enum discriminant: {}", current_variant))
                    }
                    all_variants |= 1 << current_variant;
                    if current_variant > max_variant {
                        max_variant = current_variant
                    }

                    current_variant += 1;
                } else {
                    return error(variant.span(),
                                 "`#[derive(BigEnumSetType)]` can only be used on C-like enums.")
                }
            }

            let repr = Ident::new(if max_variant <= 7 {
                "u8"
            } else if max_variant <= 15 {
                "u16"
            } else if max_variant <= 31 {
                "u32"
            } else if max_variant <= 63 {
                "u64"
            } else if max_variant <= 127 {
                "u128"
            } else {
                panic!("max_variant > 127?")
            }, Span::call_site());

            let attrs: EnumsetAttrs = match EnumsetAttrs::from_derive_input(&input) {
                Ok(attrs) => attrs,
                Err(e) => return e.write_errors().into(),
            };

            match attrs.serialize_repr.as_ref().map(|x| x.as_str()) {
                Some("u8") => if max_variant > 7 {
                    return error(input.span(), "Too many variants for u8 serialization repr.")
                }
                Some("u16") => if max_variant > 15 {
                    return error(input.span(), "Too many variants for u16 serialization repr.")
                }
                Some("u32") => if max_variant > 31 {
                    return error(input.span(), "Too many variants for u32 serialization repr.")
                }
                Some("u64") => if max_variant > 63 {
                    return error(input.span(), "Too many variants for u64 serialization repr.")
                }
                Some("u128") => if max_variant > 127 {
                    return error(input.span(), "Too many variants for u128 serialization repr.")
                }
                None => { }
                Some(x) => return error(input.span(),
                                        &format!("{} is not a valid serialization repr.", x)),
            };

            enum_set_type_impl(&input.ident, all_variants, repr, attrs).into()
        }
    } else {
        error(input.span(), "`#[derive(BigEnumSetType)]` may only be used on enums")
    }
}
