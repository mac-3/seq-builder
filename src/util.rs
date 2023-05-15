use quote::{format_ident, quote};
use syn::{parse_quote, DeriveInput, Ident, ImplItemFn, ItemImpl, ItemStruct, Type};

use crate::FieldsClassify;

/// Builds  the phases parts. In practice they're just the struct definitions.
/// For internal convention, we implement Debug, Clone, Default for each of
/// the generated struct. We might decide to implement more.
pub(crate) fn build_phases(fields: &FieldsClassify, ident: &Ident) -> Vec<ItemStruct> {
    // Required fields are the one we can't skip
    let required = fields.required().collect::<Vec<_>>();

    let mut acc = vec![];

    // Required will be handled first
    for i in 1..=required.len() {
        let ident = format_ident!("{}BuilderPhase{}", ident.to_string(), i);
        let expr: ItemStruct = parse_quote! {
            #[derive(Debug, Clone, Default)]
            pub struct #ident {}
        };

        acc.push(expr);
    }

    // Add the CanBuild for finalization
    acc.push(parse_quote! {
            #[derive(Debug, Clone, Default)]
            pub struct CanBuild {}
    });

    acc
}

pub(crate) fn build_optional_impls(fields: &FieldsClassify, ident: &Ident) -> ItemImpl {
    let mut acc: Vec<ImplItemFn> = vec![];
    let type_name = format_ident!("{}Builder", ident.to_string());

    for opt in fields.optional() {
        let f_name = opt.ident.as_ref().unwrap();
        let f_type = match &opt.ty {
            Type::Path(path) => match &path.path.segments.last().unwrap().arguments {
                syn::PathArguments::AngleBracketed(inner) => inner.args.clone(),
                _ => continue,
            },
            _ => continue,
        };
        let docs = opt
            .attrs
            .clone()
            .into_iter()
            .filter(|x| x.path().is_ident("doc"))
            .collect::<Vec<_>>();

        acc.push(parse_quote! {
            #(#docs)*
            pub fn #f_name(mut self, #f_name: impl Into<#f_type>) -> Self {
                self.#f_name.replace(#f_name.into());
                self
            }
        });

        // Add the opt variant
        let ff_name = format_ident!("{}_opt", opt.ident.as_ref().unwrap());
        acc.push(parse_quote! {
            #(#docs)*
            pub fn #ff_name(mut self, #f_name: std::option::Option<impl Into<#f_type>>) -> Self {
                self.#f_name = #f_name.map(|x| x.into());
                self
            }
        });
    }

    parse_quote!(
        impl #type_name<CanBuild> {
            #(#acc)*
        }
    )
}

pub(crate) fn build_required_impls(fields: &FieldsClassify, ident: &Ident) -> Vec<ItemImpl> {
    let mut acc = vec![];
    let required = fields.required().collect::<Vec<_>>();

    for (f, i) in required.iter().zip(1usize..) {
        let type_name = format_ident!("{}Builder", ident.to_string());
        let generic_type = format_ident!("{}BuilderPhase{}", ident.to_string(), i);
        let next_generic_type = if i != required.len() {
            format_ident!("{}BuilderPhase{}", ident.to_string(), i + 1)
        } else {
            // We handle the last one differently
            format_ident!("CanBuild")
        };
        let f_name = f.ident.as_ref().unwrap();
        let f_type = f.ty.clone();
        let relevant_idents = fields
            .exclude(f_name.to_string())
            .filter_map(|x| x.ident.clone());
        let docs = f
            .attrs
            .clone()
            .into_iter()
            .filter(|x| x.path().is_ident("doc"));

        acc.push(parse_quote! {
            impl #type_name<#generic_type> {
                #(#docs)*
                pub fn #f_name(self, #f_name: impl Into<#f_type>) -> #type_name<#next_generic_type> {
                    #type_name {
                        #(#relevant_idents: self.#relevant_idents,)*
                        #f_name: Some(#f_name.into()),
                        _p: ::std::marker::PhantomData
                    }
                }
            }
        })
    }

    acc
}

pub(crate) fn build_builder_struct(fields: &FieldsClassify, input: &DeriveInput) -> ItemStruct {
    // Construct Builder Struct
    let builder_ident = format_ident!("{}Builder", input.ident.to_string());
    let vis = &input.vis;
    let all = fields
        .all()
        .filter_map(|x| match &x.ty {
            syn::Type::Path(p) => {
                if p.path
                    .segments
                    .first()
                    .unwrap()
                    .ident
                    .to_string()
                    .eq_ignore_ascii_case("option")
                {
                    x.ident.clone().map(|id| (id, x.ty.clone()))
                } else {
                    let ty: Type = parse_quote!(Option<#p>);
                    x.ident.clone().map(|id| (id, ty))
                }
            }
            _ => None,
        })
        .map(|(id, ty)| quote!(#id: #ty));

    // Build Builder struct
    parse_quote!(
        #[derive(Debug, Clone, Default)]
        #vis struct #builder_ident<T> {
            #(#all,)*
            _p: ::std::marker::PhantomData<T>
        }
    )
}
