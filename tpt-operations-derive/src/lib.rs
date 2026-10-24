//! Derive macros for `tpt-operations`.
//!
//! `#[derive(Operation)]` implements `Operation<S>` for a struct whose named
//! fields mirror fields of the state type `S`:
//!
//! ```ignore
//! #[derive(Clone, Operation)]
//! #[tpt(state = Counter)]
//! struct SetCounter {
//!     value: i64,
//!     #[tpt(ignore)]
//!     author: String, // metadata: carried along, never written to the state
//! }
//! ```
//!
//! - `apply` writes each non-ignored field into the state field of the same name.
//! - `inverse` snapshots the pre-application state values into a new `Self`.
//!   Inverses are never written as expressions, so they cannot drift.
//! - `validate` accepts every operation. Override it by writing the trait impl by hand.
//! - `Inverse` is `Self`.
//! - `#[derive(SizeEstimate)]` implements `history::SizeEstimate` as the inline size
//!   of the type, for the `HistoryStack` memory cap.
//! - `#[tpt(commutative)]`, `#[tpt(associative)]` and `#[tpt(idempotent)]` on the
//!   struct declare the matching `CrdtCompatible` markers. Each defaults to `false`.
//!   Declare a marker only if it holds for every value of the type. Field-setting
//!   ops are idempotent but not commutative in general, so they declare
//!   `idempotent` only.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Error, Fields, Ident, Result, Type};

/// Derives `Operation` for a struct. See the crate docs for the rules.
#[proc_macro_derive(Operation, attributes(tpt))]
pub fn derive_operation(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

struct Field {
    ident: Ident,
    ignored: bool,
}

fn expand(input: &DeriveInput) -> Result<TokenStream2> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "Operation cannot be derived for generic types yet",
        ));
    }

    let (state, markers) = container_attrs(input)?;
    let Markers {
        commutative,
        associative,
        idempotent,
    } = markers;
    let (is_unit, fields) = struct_fields(input)?;
    let name = &input.ident;

    let writes = fields.iter().filter(|f| !f.ignored).map(|f| {
        let ident = &f.ident;
        quote! { state.#ident = ::core::clone::Clone::clone(&self.#ident); }
    });

    let snapshot_names: Vec<&Ident> = fields.iter().map(|f| &f.ident).collect();
    let snapshot_values = fields.iter().map(|f| {
        let ident = &f.ident;
        if f.ignored {
            quote! { ::core::clone::Clone::clone(&self.#ident) }
        } else {
            quote! { ::core::clone::Clone::clone(&state.#ident) }
        }
    });
    let constructor = if is_unit {
        quote! { Self }
    } else {
        quote! { Self { #( #snapshot_names: #snapshot_values, )* } }
    };

    Ok(quote! {
        #[allow(unused_variables)]
        impl ::tpt_operations::core::Operation<#state> for #name {
            type Inverse = Self;

            fn apply(
                &self,
                state: &mut #state,
            ) -> ::core::result::Result<(), ::tpt_operations::core::OpError> {
                #( #writes )*
                ::core::result::Result::Ok(())
            }

            fn inverse(
                &self,
                state: &#state,
            ) -> ::core::result::Result<Self, ::tpt_operations::core::OpError> {
                ::core::result::Result::Ok(#constructor)
            }

            fn validate(
                &self,
                _state: &#state,
                _actor: &::tpt_operations::core::ActorId,
            ) -> ::core::result::Result<(), ::tpt_operations::core::OpError> {
                ::core::result::Result::Ok(())
            }
        }

        impl ::tpt_operations::crdt::CrdtCompatible<#state> for #name {
            const IS_COMMUTATIVE: bool = #commutative;
            const IS_ASSOCIATIVE: bool = #associative;
            const IS_IDEMPOTENT: bool = #idempotent;
        }
    })
}

/// Derives `SizeEstimate` as `size_of::<Self>()`.
///
/// This counts the inline size only. Heap data owned by the fields, such as the
/// contents of a `String` or `Vec`, is not counted. Write the impl by hand for
/// operations that carry large heap data, so the memory cap stays honest.
/// Works for structs and enums, with or without generics.
#[proc_macro_derive(SizeEstimate)]
pub fn derive_size_estimate(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    quote! {
        impl #impl_generics ::tpt_operations::history::SizeEstimate
            for #name #type_generics #where_clause
        {
            fn estimated_size(&self) -> usize {
                ::core::mem::size_of::<Self>()
            }
        }
    }
    .into()
}

/// CRDT markers declared on the container. Each defaults to `false`, the conservative choice.
#[derive(Default)]
struct Markers {
    commutative: bool,
    associative: bool,
    idempotent: bool,
}

/// Reads the container attributes: the required `state = Type` and the optional CRDT flags.
fn container_attrs(input: &DeriveInput) -> Result<(Type, Markers)> {
    let mut state = None;
    let mut markers = Markers::default();
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("tpt")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("state") {
                let ty: Type = meta.value()?.parse()?;
                state = Some(ty);
                Ok(())
            } else if meta.path.is_ident("commutative") {
                markers.commutative = true;
                Ok(())
            } else if meta.path.is_ident("associative") {
                markers.associative = true;
                Ok(())
            } else if meta.path.is_ident("idempotent") {
                markers.idempotent = true;
                Ok(())
            } else {
                Err(meta.error(
                    "unknown container attribute; expected `state = Type`, `commutative`, `associative` or `idempotent`",
                ))
            }
        })?;
    }
    let state = state.ok_or_else(|| {
        Error::new_spanned(
            &input.ident,
            "missing `#[tpt(state = Type)]` attribute for `Operation`",
        )
    })?;
    Ok((state, markers))
}

/// Returns whether the struct is a unit struct, plus its fields in declaration order.
fn struct_fields(input: &DeriveInput) -> Result<(bool, Vec<Field>)> {
    let data = match &input.data {
        Data::Struct(data) => data,
        _ => {
            return Err(Error::new_spanned(
                &input.ident,
                "Operation can only be derived for structs",
            ))
        }
    };
    match &data.fields {
        Fields::Unit => Ok((true, Vec::new())),
        Fields::Named(named) => {
            let mut fields = Vec::new();
            for field in &named.named {
                let ident = field
                    .ident
                    .clone()
                    .ok_or_else(|| Error::new_spanned(field, "expected a named field"))?;
                fields.push(Field {
                    ident,
                    ignored: field_is_ignored(field)?,
                });
            }
            Ok((false, fields))
        }
        Fields::Unnamed(_) => Err(Error::new_spanned(
            &data.fields,
            "Operation requires named fields, or a unit struct",
        )),
    }
}

/// Reads the `#[tpt(...)]` attributes on one field.
fn field_is_ignored(field: &syn::Field) -> Result<bool> {
    let mut ignored = false;
    for attr in field.attrs.iter().filter(|a| a.path().is_ident("tpt")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("ignore") {
                ignored = true;
                Ok(())
            } else if meta.path.is_ident("inverse") {
                Err(meta.error(
                    "inverse overrides are not supported: inverses are snapshots of the pre-state",
                ))
            } else {
                Err(meta.error("unknown field attribute; expected `ignore`"))
            }
        })?;
    }
    Ok(ignored)
}
