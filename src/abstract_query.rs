use proc_macro::TokenStream;
use syn::{parse_macro_input, ExprMethodCall};

#[proc_macro]
pub fn abstract_query(input: TokenStream) -> TokenStream {
    let mut input_call_chain = parse_macro_input!(input as ExprMethodCall);
}
