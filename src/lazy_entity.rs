use proc_macro2::{TokenStream as TokenStream2, Span};
use syn::{Ident, Type, FieldValue};
use std::collections::BTreeMap;
use crate::util;

struct TypeData {
    type_name: Type,
    is_vector:bool
}

pub struct LazyEntityAST {
    original_type: Ident,
    rel_fields_lazy: Vec<syn::Field>,
    rel_default_assign_lazy: Vec<FieldValue>,
    field_type_map: BTreeMap<Ident,TypeData>,
}

impl LazyEntityAST {
    pub fn new(original_type: &Ident) -> Self {
        Self {
            original_type: original_type.clone(),
            rel_fields_lazy: Vec::new(),
            rel_default_assign_lazy: Vec::new(),
            field_type_map: BTreeMap::new(),
        }
    }
    pub fn push_many_to_one(&mut self, field_name:&Ident, type_name:&Type){
        let gen_field_name = field_name.to_string().replace("_id", "");
        let gen_field_ident =
            syn::Ident::new(&gen_field_name, Span::call_site());
        let sanitized_type = util::extract_type_from_option(&type_name);
        if util::type_is_option(type_name) {
            self.rel_fields_lazy.push(syn::parse_quote! { pub #gen_field_ident: #type_name });
        } else {
            self.rel_fields_lazy.push(syn::parse_quote! { pub #gen_field_ident: Option<#type_name> });
        }
        self.rel_default_assign_lazy.push(syn::parse_quote! { #gen_field_ident: None});
        self.field_type_map.insert(gen_field_ident.clone(), TypeData {
            type_name: sanitized_type.clone(),
            is_vector: false
        });
        
    }
    pub fn push_one_to_many(&mut self, field_name:&Ident, type_name:&Type){

        if util::type_contains(&type_name, "To") {
            let type_name_i: syn::Type = syn::parse_quote!{i64};
            self.rel_fields_lazy.push(syn::parse_quote! { pub #field_name: Option<Vec<#type_name_i>> });
            self.rel_default_assign_lazy.push(syn::parse_quote! { #field_name: None});
            self.field_type_map.insert(field_name.clone(), TypeData {
                type_name: type_name_i.clone(),
                is_vector: true
            });
        } else {
            self.rel_fields_lazy.push(syn::parse_quote! { pub #field_name: Option<Vec<#type_name>> });
            self.rel_default_assign_lazy.push(syn::parse_quote! { #field_name: None});
            self.field_type_map.insert(field_name.clone(), TypeData {
                type_name: type_name.clone(),
                is_vector: true
            });
        }
    }

    pub fn push_many_to_many(&mut self, field_name:&Ident, type_name:&Type){
        self.rel_fields_lazy.push( syn::parse_quote! { pub #field_name: Option<Vec<#type_name>> });
        self.rel_default_assign_lazy.push( syn::parse_quote! { #field_name: None});
        self.field_type_map.insert(field_name.clone(), TypeData {
            type_name: type_name.clone(),
            is_vector: true
        });
    }

    // fn make_init_with_fn(&self)->Vec<TokenStream2> {
    //     let Self {
    //         original_type,
    //         field_type_map,
    //         rel_default_assign_lazy,
    //         ..
    //     } = self;
    //     let mut ret_val: Vec<TokenStream2> = Vec::new();

    //     for (f_name, raw_type) in field_type_map {
    //         let fn_ident = util::format_ident("init_with_{}", f_name);
    //         let rel_assign: Vec<&FieldValue> = rel_default_assign_lazy.into_iter()
    //         .filter(|f_value| 
    //             if let syn::Member::Named(f_name_a) = &f_value.member {
    //                 f_name_a.to_string() != f_name.to_string()
    //             }else {
    //                 true
    //             }
    //         )
    //         .collect();
    //         ret_val.push(quote!{
    //             pub fn #fn_ident(self_data: #original_type, #f_name: #raw_type) {
    //                 Self {self_data, #f_name, #(#rel_assign),*}
    //             }
    //         })
    //     }
    //     return ret_val;
    // }

    fn make_push_or_set(&self)->Vec<TokenStream2> {
        let Self {
            original_type,
            field_type_map,
            ..
        } = self;
        let mut ret_val: Vec<TokenStream2> = Vec::new();

        for (f_name, type_data) in field_type_map {
            let fn_ident = util::format_ident("push_or_set_{}", f_name);
            let type_name = &type_data.type_name;
            if type_data.is_vector {
                ret_val.push(quote!{
                    pub fn #fn_ident(&mut self, val: #type_name) {
                        if let Some(vec) = self.#f_name.as_mut() {
                            vec.push(val);
                        }else {
                            self.#f_name = Some(vec![val]);
                        }
                    }
                });
            }else {
                ret_val.push(quote!{
                    pub fn #fn_ident(&mut self, val: #type_name) {
                        self.#f_name = Some(val);
                    }
                });
            }
        }
        return ret_val;
    }

    pub fn build(self)->TokenStream2 {
        let setters = self.make_push_or_set();
        let Self {
            original_type,
            rel_fields_lazy,
            rel_default_assign_lazy,
            ..
            } = self;
        let ident_lazy = util::format_ident("{}Lazy", &original_type);
        if rel_fields_lazy.len() == 0 {
            return quote!{};
        }
        return quote!{
            #[derive(Serialize, Clone)]
            pub struct #ident_lazy {
                #[serde(flatten)]
                pub self_data: #original_type,
                #(#rel_fields_lazy),*
            }
            impl #ident_lazy {
                pub fn init(self_data: #original_type) -> Self {
                    Self {self_data, #(#rel_default_assign_lazy),*}
                }
                #(#setters)*
            }
        };
    }
}