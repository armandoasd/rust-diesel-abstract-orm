use proc_macro2::{TokenStream as TokenStream2, Span};
use syn::{Ident, Type, Token, punctuated::Punctuated};

fn make_entity_guarded_fn(input: Punctuated::<syn::Ident, Token![.]) -> TokenStream2 {
    if input.len() == 2 {
        let table = input[0];
        let field = input[1];
        return quote!{
            .inner_join(#table::table)
            .filter(#table::#field.eq_any(roles))
        }
    }
    return quote!{}
}

// find_with_guard(guard: Vec<String>) -> {
//     quote!{
//         pub fn #find_all_with_guard(
//             conn: &mut MysqlConnection,

//         )->Vec<#ident_lazy>{
//             let mut ret_data:Vec<#ident_lazy> = Vec::new();
//             let mut last_id = 0;

//             let all_rows = #table_name::table
//                 #join_stmt
//                 .select((#original_type::as_select(), #select_type::as_select()))
//                 .load::<(#original_type, #select_type)>(conn).unwrap();

//             for (self_data, #f_name) in all_rows {
//                 let current_id = self_data.id;
//                 if last_id == current_id {
//                     let mut data = ret_data.last_mut().unwrap();
//                     #data_assign
//                 }else {
//                     let mut data = #ident_lazy::init(self_data);
//                     #data_assign;
//                     ret_data.push(data);
//                     last_id = current_id;
//                 }
//             }
        
//             return ret_data;
//         }
//     });
// }