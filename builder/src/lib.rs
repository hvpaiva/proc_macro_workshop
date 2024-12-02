use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Data, DataStruct, DeriveInput, Field, Fields, GenericArgument, Ident,
    PathArguments, Type, TypePath,
};

fn get_struct_fields(data: Data) -> Vec<Field> {
    if let Data::Struct(DataStruct {
        fields: Fields::Named(fields_named),
        ..
    }) = data
    {
        return fields_named.named.iter().cloned().collect();
    } else {
        panic!("This macro only supports structs with named fields!");
    }
}

fn is_optional_field(field: &Field) -> bool {
    is_wrapper_field(field, "Option")
}

fn is_vec_field(field: &Field) -> bool {
    is_wrapper_field(field, "Vec")
}

fn is_wrapper_field(field: &Field, wrapper_name: &str) -> bool {
    if let Type::Path(TypePath { path, .. }) = &field.ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == wrapper_name;
        }
    }
    false
}

fn get_inner_type(ty: &Type) -> Option<Type> {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                    return Some(inner_ty.clone());
                }
            }
        }
    }
    None
}

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let is_option_or_vec = |f: &Field| is_optional_field(f) || is_vec_field(f);
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let builder = Ident::new(&format!("{}Builder", name), name.span());
    let fields = get_struct_fields(input.data);
    let wrapped_fields = fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            if is_option_or_vec(f) {
                quote! {
                    #name: #ty,
                }
            } else {
                quote! {
                    #name: Option<#ty>,
                }
            }
        })
        .collect::<Vec<_>>();
    let empty_field_values = fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            if is_vec_field(f) {
                quote! {
                    #name: vec![],
                }
            } else {
                quote! {
                    #name: None,
                }
            }
        })
        .collect::<Vec<_>>();
    let build_methods = fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            let ty = &f.ty;

            if is_vec_field(f) {
                quote! {
                    fn #name(&mut self, #name: #ty) -> &mut Self {
                        self.#name = #name;
                        self
                    }
                }
            } else {
                let ty = if is_optional_field(f) {
                    get_inner_type(ty).unwrap()
                } else {
                    ty.clone()
                };
                quote! {
                    fn #name(&mut self, #name: #ty) -> &mut Self {
                        self.#name = Some(#name);
                        self
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    let build_args = fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            if is_option_or_vec(f) {
                quote! {
                    #name: self.#name.clone(),
                }
            } else {
                quote! {
                    #name: self.#name.clone().ok_or(concat!(stringify!(#name), " is required"))?,
                }
            }
        })
        .collect::<Vec<_>>();
    let token = quote! {
        struct #builder {
            #(#wrapped_fields)*
        }

        impl #name {
            pub fn builder() -> #builder {
                #builder {
                    #(#empty_field_values)*
                }
            }
        }

        impl #builder {
            #(#build_methods)*

            pub fn build(&mut self) -> Result<#name, Box<dyn std::error::Error>> {
                Ok(#name {
                    #(#build_args)*
                })
            }
        }
    };

    TokenStream::from(token)
}
