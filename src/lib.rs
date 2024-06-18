extern crate proc_macro;
extern crate syn;
#[macro_use]
extern crate quote;
extern crate proc_macro2;


use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_error::proc_macro_error;
use syn::{parse::{Parse, ParseStream}, parse_macro_input, punctuated::Punctuated, DeriveInput, Ident, Token};
use quote::ToTokens;
use std::collections::BTreeMap;

mod kw;
mod util;
mod eager_entity;
mod lazy_entity;
mod entity_implementation;

use eager_entity::{EagerEntityAST};
use lazy_entity::{LazyEntityAST};
use entity_implementation::{EntityImplAST};

struct ManyToManyAttr {
    field_name: syn::Ident,
    eq_token: Token![=],
    type_name: syn::Type,
    by_token: Option<kw::by>,
    join_type: Option<syn::Type>,
}

impl Parse for ManyToManyAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let field_name: Ident = input.parse()?;
        let eq_token: Token![=] = input.parse()?;
        let type_name:syn::Type = input.parse::<syn::Type>()?;
        let by_token: Option<kw::by> = input.parse()?;
        let join_type: Option<syn::Type> = if let Some(has_by_toen) = by_token {
            Some(input.parse::<syn::Type>()?)
        }else {
            None
        };
        Ok(ManyToManyAttr {
            field_name,
            eq_token,
            type_name,
            by_token,
            join_type
        })
    }
}

#[proc_macro_derive(Joinable, attributes(many_to_one, one_to_many, many_to_many, with_guard))]
#[proc_macro_error]
pub fn with_join(input: TokenStream) -> TokenStream {
    // Parse the string representation
    let mut ast: DeriveInput = parse_macro_input!(input);
    let original_type = ast.ident.clone();
    println!("starting work on {}", original_type);
    match &mut ast.data {
        syn::Data::Struct(ref mut struct_data) => {
            let mut eager_entity =  EagerEntityAST::new(&original_type);
            let mut lazy_entity =  LazyEntityAST::new(&original_type);
            let mut entity_impl =  EntityImplAST::new(&original_type);

            let mut save_object_fields: Vec<syn::Field> = Vec::new();
            //
            let mut table_name:Option<syn::Type> = None;

            let mut model_pk: Vec<syn::Path> = Vec::new();
            let mut model_pk_with_types: Vec<TokenStream2> = Vec::new();
            for struct_attr in ast.attrs {
                let is_one_to_many = struct_attr.meta.path().is_ident("one_to_many");
                let is_many_to_many = struct_attr.meta.path().is_ident("many_to_many");
                let is_with_guard = struct_attr.meta.path().is_ident("with_guard");
                if is_one_to_many {
                    let arguments = struct_attr
                    .meta
                    .require_list()
                    .expect("can not parse one_to_many")
                    .parse_args_with(Punctuated::<syn::TypeParam, Token![,]>::parse_terminated)
                    .expect("error parsing one_to_many as type parameters");

                    for arg in arguments {
                        if let Some(type_name) = arg.default.clone() {
                            let ident = arg.ident;
                            eager_entity.push_one_to_many(ident.clone(), type_name.clone());
                            lazy_entity.push_one_to_many(&ident, &type_name);
                            entity_impl.push_one_to_many(&ident, &type_name);
                        }
                    }
                }
                if is_many_to_many {
                    let arguments = struct_attr
                    .meta
                    .require_list()
                    .expect("can not parse many_to_many")
                    .parse_args_with(Punctuated::<ManyToManyAttr, Token![,]>::parse_terminated)
                    .expect("error parsing many_to_many as type parameters");

                    for arg in arguments {
                        let ManyToManyAttr {field_name, type_name, join_type, ..} = arg;

                        let join_type_ = if let Some(exist_join_type) = join_type {
                            exist_join_type
                        } else {
                            let main_t = ast.ident.to_string();
                            let mut type_str = format!("{}", quote!(#type_name));
                            type_str = type_str.replace(&main_t, "");
                            let dest_t = format!("{}To{}", main_t, type_str);
                            syn::parse2::<syn::Type>(dest_t.parse().unwrap()).expect("can not find model type")
                        };
                        lazy_entity.push_many_to_many(&field_name.clone(), &type_name.clone());
                        entity_impl.push_many_to_many(&field_name.clone(), &type_name.clone(), &join_type_.clone());
                        eager_entity.push_many_to_many(&field_name.clone(), &type_name.clone(), &join_type_.clone());
                    }
                }
                if is_with_guard {
                    let query_path = struct_attr
                    .meta
                    .require_list()
                    .expect("can not parse with_guard")
                    .parse_args_with(Punctuated::<syn::Ident, Token![.]>::parse_terminated)
                    .expect("error parsing with_guard as type parameters");

                    println!("query path guards: {:?}", query_path);
                }
                if struct_attr.meta.path().is_ident("diesel"){
                    if let Ok(arguments) = struct_attr.meta.require_list() {
                        entity_impl.parse_diesel_attr(&arguments);
                    }
                }
            }
            match &mut struct_data.fields {
                syn::Fields::Named(fields) => {
                    fields.named.clone().into_iter().for_each(|f| {
                        if let Some(field_name) = f.ident.clone() {
                            let mut field_data = f.clone();
                            for attr in f.attrs {
                                if attr.meta.path().is_ident("many_to_one") {
                                    let argument = attr
                                        .meta
                                        .require_list()
                                        .expect("error parsing macro many_to_one parameters")
                                        .parse_args::<syn::Type>()
                                        .expect("error parsing macro type");

                                    eager_entity.push_many_to_one(&field_name, argument.clone());
                                    lazy_entity.push_many_to_one(&field_name, &argument);
                                    entity_impl.push_many_to_one(&field_name, &argument);
                                }
                            }
                            entity_impl.search_pk_type(&field_name, &f.ty);
                            if field_name != "id" {
                                field_data.attrs.retain(|attr| !attr.meta.path().is_ident("many_to_one"));
                                save_object_fields.push(field_data);
                            }
                        }
                    });
                }
                _ => {},
            }

            let eager_entity_ast = eager_entity.build();

            let lazy_entity_ast = lazy_entity.build();

            let entity_impl_ast = entity_impl.build();

            let ident_save = util::format_ident("New{}", &ast.ident);

            let get_for = if original_type.to_string().contains("To") {
                let pk1 = save_object_fields[0].ident.clone().unwrap();
                let pk2 = save_object_fields[1].ident.clone().unwrap();
                let get_for_pk1 = util::format_ident("get_for_{}", &pk1);
                let get_for_pk2 = util::format_ident("get_for_{}", &pk2);
                quote!{
                    impl #original_type {
                        pub fn #get_for_pk1(&self) -> i64 {
                            self.#pk2
                        }
                        pub fn #get_for_pk2(&self) -> i64 {
                            self.#pk1
                        }
                    }
                }
            }else {
                quote!{}
            };

            let table_ref_quote = if let Some(table_name_) = &entity_impl.table_name.clone() {
                quote!{
                    #[derive(diesel::Insertable, Serialize, Deserialize)]
                    #[diesel(table_name = #table_name_)]
                    pub struct #ident_save {
                        #(#save_object_fields),*
                    }
                }
            } else {
                println!("no table name was found");
                quote!{}
            };

            let ret_value = quote! {
                #table_ref_quote
                #eager_entity_ast
                #entity_impl_ast
                #lazy_entity_ast
                #get_for
            };
            
            println!("macro {}", ret_value);
            return ret_value
            .into();
        }
        _ => panic!("Jinable has to be used with structs"),
    }
}

struct FetchTree {
    model: syn::Type,
    ident: syn::Ident,
    load: Vec<syn::Ident>
}

impl FetchTree {
    fn new(ident:syn::Ident,model:syn::Type)-> Self {
        Self {
            model,
            ident,
            load: Vec::new()
        }
    }
}

#[proc_macro]
pub fn lazy_block(input: TokenStream) -> TokenStream {
    let mut block_statements = parse_macro_input!(input with syn::Block::parse_within);
    let mut model_assignments: BTreeMap<syn::Ident, FetchTree> = BTreeMap::new();
    
    for statement in &block_statements {
        match statement {
            syn::Stmt::Local(let_stmt) => {
                match &let_stmt.pat {
                    syn::Pat::Ident(ident_path) => {
                        println!("let with no type assign");
                        if let Some(init) = &let_stmt.init {
                            if let syn::Expr::Call(ref call_expr) = *init.expr {
                                println!("init Call expr");
                                if let syn::Expr::Path(ref path_expr) = *call_expr.func {
                                    let mut new_path = &mut path_expr.path.segments.clone();
                                    if let Some(method) = new_path.last() {
                                        if method.ident == "find" {
                                            println!("init path call with find method");
                                            new_path.pop();
                                            new_path.pop_punct();
                                        }
                                    }
                                    
                                    println!("init path call from {}", quote!{#new_path});
                                    let new_path_to_stream = new_path.to_token_stream();
                                    let generated_type:syn::Type = syn::parse2(new_path_to_stream).expect("can not find model type");
                                }
                            }
                        }
                    },
                    syn::Pat::Type(pat_type) => {
                        if let syn::Pat::Ident(ref ident_path) = *pat_type.pat {
                            let ident_str = &ident_path.ident.to_string();
                            println!("let with type assign");
                            model_assignments.insert(ident_path.ident.clone(), FetchTree::new(ident_path.ident.clone(), *pat_type.ty.clone()));
                        }
                    },
                    _ => {}
                }
            },
            syn::Stmt::Expr(expr_stmt, semi) => {
                match expr_stmt {
                    // syn::ExprField(field_access) => {

                    // },
                    _=>{}
                }
            }
            _=>{}
        }
    }
    
    let ret_value = quote!{
        #(#block_statements)*
    };
    println!("lazy block macro {}", ret_value);
    return ret_value.into();
}