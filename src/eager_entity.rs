use proc_macro2::{TokenStream as TokenStream2, Span};
use syn::{Ident, Type};
use crate::util;

pub struct EagerEntityAST {
    original_type: Ident,
    eager_rel_fields: Vec<TokenStream2>,
    rel_params: Vec<Ident>,
    rel_params_t: Vec<Type>,
    many_to_one_params_t: Vec<Type>,
    many_to_one_params: Vec<Ident>,
    eager_rel_default_assign: Vec<TokenStream2>,
    one_to_many_data_assign: Vec<TokenStream2>,
}

impl EagerEntityAST {
    pub fn new(original_type: &Ident) -> Self {
        Self {
            original_type: original_type.clone(),
            eager_rel_fields: Vec::new(),
            rel_params: Vec::new(),
            rel_params_t: Vec::new(),
            many_to_one_params_t: Vec::new(),
            many_to_one_params: Vec::new(),
            eager_rel_default_assign: Vec::new(),
            one_to_many_data_assign: Vec::new(),
        }
    }
    pub fn push_many_to_one(&mut self, field_name:&Ident, type_name:Type){
        let gen_field_name = field_name.to_string().replace("_id", "");
        let gen_field_ident =
            syn::Ident::new(&gen_field_name, Span::call_site());
        self.rel_params.push(gen_field_ident.clone());
        self.rel_params_t.push(type_name.clone());
        self.eager_rel_default_assign.push(quote!{#gen_field_ident,});
        self.eager_rel_fields.push(quote! { pub #gen_field_ident: #type_name, });
    }
    pub fn push_one_to_many(&mut self, field_name:Ident, type_name:Type){
        self.rel_params.push(field_name.clone());
        let optional_type:Type = syn::parse_quote!{Option<#type_name>};
        self.rel_params_t.push(optional_type);
        self.eager_rel_fields.push(quote! { pub #field_name: Vec<#type_name>, });
        self.eager_rel_default_assign.push(quote! { #field_name: if let Some(data) = #field_name { vec![data] } else { Vec::new() },});
        self.one_to_many_data_assign.push(quote! {
            if let Some(data) = #field_name {
                self.#field_name.push(data);
            }
        });
    }

    pub fn push_many_to_many(&mut self, field_name:&Ident, type_name:&Type, join_type: &Type){
        self.push_one_to_many(field_name.clone(), type_name.clone());
    }

    fn sort_many_to_one_params(&mut self){
        // let mut many_to_one_params_zip:Vec<(Ident,Type)> = self.many_to_one_params.drain(..).zip(self.many_to_one_params_t.drain(..)).collect();
        // many_to_one_params_zip.sort_by(|(a,_), (b,_)| a.to_string().cmp(&b.to_string()));
        // let (mut many_to_one_params_sorted, mut many_to_one_params_t_sorted) = many_to_one_params_zip.into_iter().unzip();
        // self.rel_params.append(&mut many_to_one_params_sorted);
        // self.rel_params_t.append(&mut many_to_one_params_t_sorted);
    }

    pub fn prepare(&mut self){
        self.sort_many_to_one_params();
    }

    pub fn build(self)->TokenStream2 {
        let Self {original_type, eager_rel_fields, rel_params, rel_params_t, eager_rel_default_assign, one_to_many_data_assign, ..} = self;
        let ident_with_all = util::format_ident("{}WithAll", &original_type);
        if eager_rel_fields.len() == 0 {
            return quote!{};
        }
        return quote!{
            #[derive(Serialize, Clone)]
            pub struct #ident_with_all {
                #[serde(flatten)]
                pub self_data: #original_type,
                #(#eager_rel_fields)*
            }
            impl #ident_with_all {
                pub fn new_builder((self_data, #(#rel_params),*):(#original_type,#(#rel_params_t),* )) -> Self {
                    Self {self_data, #(#eager_rel_default_assign)*}
                }
                pub fn insert_data(&mut self, (self_data, #(#rel_params),*):(#original_type,#(#rel_params_t),* )){
                    #(#one_to_many_data_assign)*
                }
            }
        };
    }
}