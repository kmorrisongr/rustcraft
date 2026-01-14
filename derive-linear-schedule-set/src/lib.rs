extern crate proc_macro;
extern crate proc_macro2;
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// Derive macro for generating a `chained_schedule_configs()` method on enums used as Bevy SystemSets.
///
/// # Usage
/// ```ignore
/// #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash, LinearSystemSet)]
/// pub enum OnEnterGame {
///    Initialize,
///    Ui,
///    Rest,
/// }
/// ```
///
/// Generates:
/// ```ignore
/// impl OnEnterGame {
///    pub fn chained_schedule_configs() -> ScheduleConfigs<InternedSystemSet> {
///        (
///            OnEnterGame::Initialize,
///            OnEnterGame::Ui,
///            OnEnterGame::Rest,
///        ).chain()
///    }
/// }
/// ```
#[proc_macro_derive(LinearSystemSet)]
pub fn linear_system_set_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Extract enum variants
    let variants = match &input.data {
        Data::Enum(data_enum) => &data_enum.variants,
        _ => {
            return syn::Error::new_spanned(
                &input,
                "LinearSystemSet can only be derived for enums",
            )
            .to_compile_error()
            .into();
        }
    };

    // Verify all variants are unit variants (no fields)
    for variant in variants.iter() {
        if !matches!(variant.fields, Fields::Unit) {
            return syn::Error::new_spanned(
                variant,
                "LinearSystemSet only supports unit variants (no fields)",
            )
            .to_compile_error()
            .into();
        }
    }

    // Build the tuple of all variants: (Enum::A, Enum::B, Enum::C)
    let variant_idents: Vec<_> = variants.iter().map(|v| &v.ident).collect();

    let expanded = quote! {
        impl #name {
            pub fn chained_schedule_configs() -> ::bevy::ecs::schedule::ScheduleConfigs<::bevy::ecs::schedule::InternedSystemSet> {
                use ::bevy::prelude::IntoScheduleConfigs;
                (
                    #( #name::#variant_idents, )*
                ).chain()
            }
        }
    };

    expanded.into()
}
