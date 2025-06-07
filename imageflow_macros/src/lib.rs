extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::{quote, format_ident};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Attribute,
    Expr, ExprLit, FnArg, ItemFn, Lit, Meta, Pat, PatType, Result, ReturnType, Token, Type, TypePath,
    punctuated::Punctuated,
    visit::Visit,
};

use imageflow_types::{EndpointDefinition, HttpMethod, submit_endpoint};

// Helper to parse key-value attributes like path = "/v1/build"
struct EndpointAttribute {
    key: Ident,
    value: Lit,
}

impl Parse for EndpointAttribute {
    fn parse(input: ParseStream) -> Result<Self> {
        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let value: Lit = input.parse()?;
        Ok(EndpointAttribute { key, value })
    }
}

#[proc_macro_attribute]
pub fn endpoint(attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let func_name = &func.sig.ident;

    // Parse attributes provided to the macro (e.g., #[endpoint(path = "/v1/foo", method = POST)])
    let attrs = parse_macro_input!(attr with Punctuated<EndpointAttribute, Token![,]>::parse_terminated);

    let mut path: Option<String> = None;
    let mut method: Option<HttpMethod> = None;
    let mut tags: Vec<String> = Vec::new(); // TODO: Support tags attribute
    let mut summary: Option<String> = None; // TODO: Support summary attribute
    let mut description: Option<String> = None; // TODO: Support description attribute
    let mut operation_id: Option<String> = None; // TODO: Support operation_id attribute

    for attribute in attrs {
        let key = attribute.key.to_string();
        match key.as_str() {
            "path" => {
                if let Lit::Str(lit_str) = attribute.value {
                    path = Some(lit_str.value());
                } else {
                    return TokenStream::from(
                        syn::Error::new_spanned(attribute.value, "Expected string literal for path").to_compile_error(),
                    );
                }
            }
            "method" => {
                if let Lit::Str(lit_str) = attribute.value {
                    let method_str = lit_str.value().to_uppercase();
                    method = match method_str.as_str() {
                        "GET" => Some(HttpMethod::Get),
                        "POST" => Some(HttpMethod::Post),
                        "PUT" => Some(HttpMethod::Put),
                        "DELETE" => Some(HttpMethod::Delete),
                        "PATCH" => Some(HttpMethod::Patch),
                        "HEAD" => Some(HttpMethod::Head),
                        "OPTIONS" => Some(HttpMethod::Options),
                        "TRACE" => Some(HttpMethod::Trace),
                        _ => {
                            return TokenStream::from(
                                syn::Error::new_spanned(
                                    lit_str,
                                    format!("Unsupported HTTP method: {}", method_str),
                                )
                                .to_compile_error(),
                            );
                        }
                    };
                } else {
                    return TokenStream::from(
                        syn::Error::new_spanned(attribute.value, "Expected string literal for method (e.g., \"POST\")")
                            .to_compile_error(),
                    );
                }
            }
            // TODO: Parse other attributes like tags, summary, description, operation_id
            _ => {
                return TokenStream::from(
                    syn::Error::new_spanned(attribute.key, format!("Unknown attribute: {}", key))
                        .to_compile_error(),
                );
            }
        }
    }

    // --- Validate required attributes --- 
    let path = match path {
        Some(p) => p,
        None => return TokenStream::from(
            syn::Error::new(Span::call_site(), "Missing required attribute: path = \"/your/path\"").to_compile_error(),
        ),
    };
    let method = match method {
        Some(m) => m,
        None => return TokenStream::from(
            syn::Error::new(Span::call_site(), "Missing required attribute: method = \"POST\" (or GET, etc.)").to_compile_error(),
        ),
    };

    // --- Analyze function signature --- 
    let mut request_type: Option<Type> = None;
    let mut response_type: Option<Type> = None;
    let mut context_arg_name: Option<Ident> = None;
    let mut request_arg_name: Option<Ident> = None;
    
    // Expected signature pattern: 
    // fn my_handler(context: &mut Context, data: RequestType) -> Result<ResponseType>
    // Or for static handlers:
    // fn my_static_handler(data: RequestType) -> Result<ResponseType>
    // fn my_static_handler_no_args() -> Result<ResponseType>

    let inputs = &func.sig.inputs;
    if inputs.len() > 2 {
         return TokenStream::from(
            syn::Error::new_spanned(&func.sig.inputs, "Endpoint function expects at most 2 arguments: [context: &mut Context], [data: RequestType]").to_compile_error(),
        );
    }

    // Identify context and request type from arguments
    for arg in inputs {
        if let FnArg::Typed(PatType { pat, ty, .. }) = arg {
             if let Pat::Ident(pat_ident) = &**pat {
                // Check if it looks like the Context argument
                if let Type::Reference(type_ref) = &**ty {
                    if let Type::Path(type_path) = &*type_ref.elem {
                        if type_path.path.segments.last().map_or(false, |seg| seg.ident == "Context") {
                             if context_arg_name.is_some() {
                                return TokenStream::from(syn::Error::new_spanned(ty, "Duplicate Context argument").to_compile_error());
                            }
                            context_arg_name = Some(pat_ident.ident.clone());
                            continue; // Skip to next arg
                        }
                    }
                }
                
                // If not Context, assume it's the request data argument
                 if request_type.is_some() || request_arg_name.is_some() {
                    return TokenStream::from(syn::Error::new_spanned(ty, "Duplicate request data argument").to_compile_error());
                }
                request_type = Some((**ty).clone());
                request_arg_name = Some(pat_ident.ident.clone());
            }
        } else {
             return TokenStream::from(syn::Error::new_spanned(arg, "Unsupported argument type in endpoint function signature (expected typed identifier)").to_compile_error());
        }
    }

    // Extract response type from return type Result<ResponseType>
    if let ReturnType::Type(_, ty) = &func.sig.output {
        if let Type::Path(TypePath { path, .. }) = &**ty {
            if let Some(segment) = path.segments.last() {
                if segment.ident == "Result" {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            response_type = Some(inner_ty.clone());
                        } else {
                             return TokenStream::from(syn::Error::new_spanned(&segment.arguments, "Expected Result<ResponseType>").to_compile_error());
                        }
                    } else {
                         return TokenStream::from(syn::Error::new_spanned(&segment.arguments, "Expected Result<ResponseType>").to_compile_error());
                    }
                } else {
                     return TokenStream::from(syn::Error::new_spanned(&segment.ident, "Expected return type Result<...>)").to_compile_error());
                }
            } else {
                 return TokenStream::from(syn::Error::new_spanned(path, "Expected return type Result<...>)").to_compile_error());
            }
        } else {
            return TokenStream::from(syn::Error::new_spanned(ty, "Expected return type Result<...>)").to_compile_error());
        }
    } else {
        return TokenStream::from(syn::Error::new_spanned(&func.sig.output, "Endpoint function must return Result<ResponseType>").to_compile_error());
    }

    // --- TODO: Generate Code --- 
    // 1. Generate static functions for schema access if types are present.
    // 2. Generate the `inventory::submit!` call with an `EndpointDefinition` instance.
    // 3. Output the original function definition unmodified.

    // For now, just output the original function
    let output = quote! {
        #func
    };

    TokenStream::from(output)
} 