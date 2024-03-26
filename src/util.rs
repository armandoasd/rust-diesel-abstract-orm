use proc_macro2::Span;
use syn::{Type, Ident, Path, GenericArgument, PathArguments};
use convert_case::{Case, Casing};

pub fn format_ident(format:&str, ident:&Ident) -> Ident {
    let format_s = format.to_string();
    return Ident::new(&format_s.replace("{}", &ident.to_string()), Span::call_site());
}
pub fn format_ident_snake(format:&str, ident:&Ident) -> Ident {
    let format_s = format.to_string();
    return Ident::new(&format_s.replace("{}", &ident.to_string().to_case(Case::Snake)), Span::call_site());
}

fn path_is_option(path: &Path) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments.iter().next().unwrap().ident == "Option"
}

pub fn type_is_option(type_name: &Type) -> bool {
    if let Type::Path(typepath) = type_name {
        return typepath.qself.is_none() && path_is_option(&typepath.path);
    }
    return false;
}

pub fn extract_type_from_option(ty: &Type) -> Type {
    match ty {
        Type::Path(typepath) if typepath.qself.is_none() && path_is_option(&typepath.path) => {
            // Get the first segment of the path (there is only one, in fact: "Option"):
            let type_params = typepath.path.segments.first().unwrap().arguments.clone();
            // It should have only on angle-bracketed param ("<String>"):
            match type_params {
                PathArguments::AngleBracketed(params) => {
                    let generic_arg = params.args.first().unwrap();
                    match generic_arg {
                        GenericArgument::Type(ty) => ty.clone(),
                        _ => panic!("TODO: error handling"),
                    }
                },
                _ => panic!("TODO: error handling"),
            }
        }
        _ => ty.clone(),
    }
}

pub fn make_type_option(ty: &Type) -> Type {
    return syn::parse_quote!{Option::<#ty>};
}