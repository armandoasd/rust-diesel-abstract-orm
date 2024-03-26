use proc_macro2::{TokenStream as TokenStream2, Span};
use syn::{Ident, Type, Token, punctuated::Punctuated};
use crate::util;
use std::collections::BTreeMap;

pub struct EntityImplAST {
    original_type: Ident,
    rel_fields_lazy_get: Vec<TokenStream2>,
    pub table_name:Option<Type>,
    join_statements: BTreeMap<Ident,TokenStream2>,
    rel_types_map: BTreeMap<Ident,Type>,
    model_pk: Vec<Ident>,
    model_pk_t: BTreeMap<Ident,Type>,
    rel_collect_types: Vec<Type>
}

impl EntityImplAST {
    pub fn new(original_type: &Ident) -> Self {
        Self {
            original_type: original_type.clone(),
            rel_fields_lazy_get: Vec::new(),
            table_name: None,
            model_pk: Vec::new(),
            model_pk_t: BTreeMap::new(),
            join_statements: BTreeMap::new(),
            rel_types_map: BTreeMap::new(),
            rel_collect_types: Vec::new(),
        }
    }
    
    pub fn set_table_name(&mut self, table_name:syn::Type){
        self.table_name = Some(table_name);
    }

    pub fn parse_diesel_attr(&mut self, meta_args: &syn::MetaList){
        if let Ok(arguments_parsed) = meta_args.parse_args_with(Punctuated::<syn::TypeParam, Token![,]>::parse_terminated) {
            for arg in arguments_parsed {
                if arg.ident.to_string().eq("table_name"){
                    if let Some(type_name) = arg.default.clone() {
                        self.set_table_name(type_name);
                    }
                }
            }
        }
        if let Ok(argument_parsed) = meta_args.parse_args::<syn::ExprCall>() {
            if let syn::Expr::Path(function_path) = *argument_parsed.func {
                if function_path.path.is_ident("primary_key") {
                    for f_arg in argument_parsed.args {
                        if let syn::Expr::Path(f_arg_path) = f_arg {
                            if let Some(path_ident) = f_arg_path.path.get_ident(){
                                self.model_pk.push(path_ident.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn search_pk_type(&mut self, field_name: &Ident, type_value: &Type ){
        for pk_ident in &self.model_pk {
            if pk_ident == field_name {
                self.model_pk_t.insert(pk_ident.clone(), type_value.clone());
            }
        }
    }

    pub fn push_many_to_one(&mut self, field_name:&Ident, type_name:&Type){
        let sanitized_type = util::extract_type_from_option(&type_name);
        let gen_field_name = field_name.to_string().replace("_id", "");
        let gen_field_ident =
            syn::Ident::new(&gen_field_name, Span::call_site());
        let get_ident = util::format_ident("get_{}", &gen_field_ident);
        if util::type_is_option(type_name) {
            self.rel_fields_lazy_get.push(quote! { 
                pub fn #get_ident(&self, conn: &mut MysqlConnection) -> #type_name {
                    if let Some(fk_value) = self.#field_name {
                        return #sanitized_type::find(fk_value, conn).expect("could not fetch relation");
                    } else {
                        return None;
                    }
                }
            });
            self.join_statements.insert(gen_field_ident.clone(), quote!{
                .left_join(#sanitized_type::get_table_ref())
            });
            self.rel_collect_types.push(util::make_type_option(&sanitized_type));
            self.rel_types_map.insert(gen_field_ident.clone(), util::make_type_option(&sanitized_type));
        } else {
            self.rel_fields_lazy_get.push(quote! { 
                pub fn #get_ident(&self, conn: &mut MysqlConnection) -> #type_name {
                    return #sanitized_type::find(self.#field_name, conn).expect("could not fetch relation").expect("related instance does not exist");
                }
            });
            self.join_statements.insert(gen_field_ident.clone(), quote!{
                .inner_join(#sanitized_type::get_table_ref())
            });
            self.rel_collect_types.push(type_name.clone());
            self.rel_types_map.insert(gen_field_ident.clone(), type_name.clone());
        }
    }
    pub fn push_one_to_many(&mut self, field_name:&Ident, type_name:&Type){
        let get_ident = util::format_ident("get_{}", field_name);
        self.rel_fields_lazy_get.push(quote! { 
            pub fn #get_ident(&self, conn: &mut MysqlConnection) -> Vec<#type_name> {
                return #type_name::belonging_to(&self)
                .select(#type_name::as_select())
                .load(conn).expect("error fetching #ident from #type_name");
            }
        });
        self.rel_collect_types.push(util::make_type_option(type_name));
        self.join_statements.insert(field_name.clone(), quote!{
            .left_join(#type_name::get_table_ref())
        });
        self.rel_types_map.insert(field_name.clone(), util::make_type_option(type_name));
    }

    pub fn push_many_to_many(&mut self, field_name:&Ident, type_name:&Type, join_type: &Type){
        let get_ident = util::format_ident("get_{}", &field_name);
        self.rel_fields_lazy_get.push(quote! { 
            pub fn #get_ident(&self, conn: &mut MysqlConnection) -> Vec<#type_name> {
                return #join_type::belonging_to(&self)
                .inner_join(#type_name::get_table_ref())
                .select(#type_name::as_select())
                .load(conn).expect("error running query to fetch many to many relationship");
            }
        });
        self.rel_collect_types.push(util::make_type_option(type_name));
        //self.rel_collect_types.push(util::make_type_option(join_type));
        self.join_statements.insert(field_name.clone(), quote!{
            .left_join(#join_type::get_table_ref().left_join(#type_name::get_table_ref()))
        });
        self.rel_types_map.insert(field_name.clone(), util::make_type_option(type_name));
    }

    fn make_find_fn(&self, table_name: &Type)->TokenStream2 {
        if self.model_pk_t.len() > 0 {
            let mut find_params: Vec<TokenStream2> = Vec::new();
            for (k, v) in &self.model_pk_t {
                find_params.push(quote!{#k:#v});
            }
            let model_pk = &self.model_pk;
            return quote!{
                pub fn find(
                    #(#find_params),*,
                    conn: &mut MysqlConnection,
                ) -> Result<Option<Self>, diesel::result::Error> {
                    
                    let result = #table_name::table
                        #(.filter(#table_name::#model_pk.eq(#model_pk)))*
                        .first::<Self>(conn)
                        .optional()?;
                        
                    Ok(result)
                }
            };
        }else {
            return Self::default_find_fn(&table_name);
        }
    }

    fn default_find_fn(table_name: &Type)->TokenStream2 {
        return quote!{
            pub fn find(
                uid: i64,
                conn: &mut MysqlConnection,
            ) -> Result<Option<Self>, diesel::result::Error> {
                
                let result = #table_name::table
                    .filter(#table_name::id.eq(uid))
                    .first::<Self>(conn)
                    .optional()?;
                    
                Ok(result)
            }
        };
    }

    fn make_find_all_eager_fn(&self, table_name: &Type)->TokenStream2 {
        let Self {original_type, join_statements, rel_collect_types, model_pk, ..} = self;
        let ident_with_all = util::format_ident("{}WithAll", &original_type);

        let join_stmts: Vec<TokenStream2> = join_statements.clone().into_values().collect();
        println!("join statements {:?}", join_stmts.clone().into_iter().map(|t| format!("{}", t)).collect::<String>());
        if join_stmts.len() > 0 && model_pk.len() == 0 {
            return quote!{
                pub fn find_all_eager(
                    conn: &mut MysqlConnection
                )->Vec<#ident_with_all>{
                    use crate::schema::*;
                
                    let mut ret_data:Vec<#ident_with_all> = Vec::new();
                    let mut last_id = 0;
                
                    let all_rows = #table_name::table
                        #(#join_stmts)*
                        .select((#original_type::as_select(), #(#rel_collect_types::as_select()),*))
                        .load::<(#original_type, #(#rel_collect_types),*)>(conn).unwrap();
                
                    
                    for query_row in all_rows {
                        let current_id = query_row.0.id;
                        if last_id == current_id {
                            ret_data.last_mut().unwrap().insert_data(query_row);
                        }else {
                            let data = #ident_with_all::new_builder(query_row);
                            ret_data.push(data);
                            last_id = current_id;
                        }
                    }
                
                    return ret_data;
                }
            }
        }else {
            return quote!{};
        }

    }

    fn make_find_all_with(&self, table_name: &Type)->Vec<TokenStream2> {
        let Self {original_type, join_statements, rel_collect_types, model_pk, ..} = self;
        let ident_lazy = util::format_ident("{}Lazy", &original_type);
        let mut output: Vec<TokenStream2> =  Vec::new();
        if model_pk.len() == 0 {
            for (f_name, join_stmt) in join_statements {
                let fn_ident = util::format_ident("find_all_with_{}", &f_name);
                let set_fn_ident = util::format_ident("push_or_set_{}", &f_name);
                let select_type = self.rel_types_map.get(&f_name).unwrap();
            
                let data_assign = if util::type_is_option(select_type) {
                    quote!{
                        if let Some(val) = #f_name {
                            data.#set_fn_ident(val);
                        }
                    }
                }else {
                    quote!{
                        data.#set_fn_ident(#f_name);
                    }
                };
                output.push(quote!{
                    pub fn #fn_ident(
                        conn: &mut MysqlConnection
                    )->Vec<#ident_lazy>{
                        let mut ret_data:Vec<#ident_lazy> = Vec::new();
                        let mut last_id = 0;
    
                        let all_rows = #table_name::table
                            #join_stmt
                            .select((#original_type::as_select(), #select_type::as_select()))
                            .load::<(#original_type, #select_type)>(conn).unwrap();
    
                        for (self_data, #f_name) in all_rows {
                            let current_id = self_data.id;
                            if last_id == current_id {
                                let mut data = ret_data.last_mut().unwrap();
                                #data_assign
                            }else {
                                let mut data = #ident_lazy::init(self_data);
                                #data_assign;
                                ret_data.push(data);
                                last_id = current_id;
                            }
                        }
                    
                        return ret_data;
                    }
                });
            }
        }

        return output;
    }

    pub fn build(&self)->TokenStream2 {
        let Self {
            original_type,
            rel_fields_lazy_get,
            ..
            } = &self;
        let ident_save = util::format_ident("New{}", &original_type);

        
        if let Some (table_name) = &self.table_name {
            let find_fn = &self.make_find_fn(&table_name);
            let find_all_eager = &self.make_find_all_eager_fn(&table_name);
            let find_all_with = &self.make_find_all_with(&table_name);
            return quote!{
                impl #original_type {
                    pub fn get_table_ref() -> #table_name::table {
                        return #table_name::table;
                    }
                    pub fn find_all(
                        conn: &mut MysqlConnection,
                    )->Vec<Self>{
                        return #table_name::table
                            .select(Self::as_select())
                            .load(conn)
                            .unwrap();
                    }
                    pub fn insert(
                        data: #ident_save,
                        conn: &mut MysqlConnection,
                    ) -> Result<#ident_save, diesel::result::Error> {                        
                        diesel::insert_into(#table_name::table).values(&data).execute(conn)?;
                    
                        Ok(data)
                    }
                    #find_fn
                    #find_all_eager
                    #(#rel_fields_lazy_get)*
                    #(#find_all_with)*
                }
            };
        }else {
            return quote!{};
        }
    }
}