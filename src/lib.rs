mod util;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Field, Fields, Type};
use util::{build_builder_struct, build_optional_impls, build_phases, build_required_impls};

struct FieldsClassify {
    fields: Vec<Field>,
}

impl FieldsClassify {
    pub fn new(fields: Fields) -> Self {
        Self {
            fields: fields.into_iter().collect::<Vec<_>>(),
        }
    }

    pub fn all(&self) -> impl Iterator<Item = &Field> {
        self.fields.iter()
    }

    pub fn required(&self) -> impl Iterator<Item = &Field> {
        self.fields.iter().filter(|x| {
            match &x.ty {
                syn::Type::Path(path) => {
                    if let Some(p) = path.path.segments.first() {
                        return !p.ident.to_string().eq_ignore_ascii_case("option");
                    }
                }
                _ => (),
            }
            false
        })
    }

    pub fn exclude(&self, s: String) -> impl Iterator<Item = &Field> {
        self.fields.iter().filter_map(move |x| {
            x.ident
                .as_ref()
                .filter(|x| !x.to_string().eq_ignore_ascii_case(&s))
                .map(|_| x)
        })
    }

    pub fn optional(&self) -> impl Iterator<Item = &Field> {
        self.fields.iter().filter(|x| {
            match &x.ty {
                syn::Type::Path(path) => {
                    if let Some(p) = path.path.segments.first() {
                        return p.ident.to_string().eq_ignore_ascii_case("option");
                    }
                }
                _ => (),
            }
            false
        })
    }
}

#[proc_macro_derive(Send)]
pub fn derive_send(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match &input.data {
        syn::Data::Struct(s) => {
            let fields = FieldsClassify::new(s.fields.clone());

            // Build phase
            let phases = build_phases(&fields, &input.ident);

            // Build required impls
            let required_impls = build_required_impls(&fields, &input.ident);

            // Builder Builder struct
            let builder_struct = build_builder_struct(&fields, &input);

            // Build construction impl
            let item_id = input.ident.clone();

            let all = fields.all().filter_map(|x| match &x.ty {
                Type::Path(p) => {
                    let id = x.ident.as_ref().unwrap();
                    if p.path
                        .segments
                        .first()
                        .unwrap()
                        .ident
                        .to_string()
                        .eq_ignore_ascii_case("option")
                    {
                        Some(quote!(#id: self.#id))
                    } else {
                        Some(quote!(#id: self.#id.unwrap()))
                    }
                }
                _ => None,
            });

            let builder_ident = builder_struct.ident.clone();
            let construct_impl = quote!(impl #builder_ident<CanBuild> {
                pub fn build(self) -> #item_id {
                    #item_id {
                        #(#all,)*
                    }
                }
            });

            // Prepare impl for type
            let first_phase = phases.first().unwrap().ident.clone();
            let vis = input.vis;
            let item_impl = quote!(
                impl #item_id {
                    #vis fn builder() -> #builder_ident<#first_phase> {
                        #builder_ident::default()
                    }
                }
            );

            // Optional building
            let optional_impls = build_optional_impls(&fields, &input.ident);

            quote!(
                #item_impl

                #builder_struct

                #(#phases)*

                #(#required_impls)*
                #optional_impls

                #construct_impl
            )
            .into()
        }
        _ => quote!({}).into(),
    }
}
