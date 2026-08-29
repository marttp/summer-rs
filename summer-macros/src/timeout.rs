use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, ItemFn, LitStr, ReturnType, Token, Type};

struct TimeoutArgs {
    name: LitStr,
}

impl Parse for TimeoutArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        while !input.is_empty() {
            let key = input.parse::<Ident>()?;
            input.parse::<Token![=]>()?;
            match key.to_string().as_str() {
                "name" => {
                    let value = input.parse::<LitStr>()?;
                    if name.replace(value).is_some() {
                        return Err(syn::Error::new_spanned(key, "duplicate `name` parameter"));
                    }
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        key,
                        "unknown timeout parameter; expected `name`",
                    ));
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        let name = name.ok_or_else(|| input.error("missing required `name` parameter"))?;
        if name.value().is_empty() {
            return Err(syn::Error::new_spanned(
                name,
                "timeout policy name cannot be empty",
            ));
        }
        Ok(Self { name })
    }
}

pub fn timeout(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as TimeoutArgs);
    let function = syn::parse_macro_input!(item as ItemFn);
    expand(args, function)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(args: TimeoutArgs, function: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    if function.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            function.sig.fn_token,
            "#[timeout] only supports async functions",
        ));
    }
    if function.sig.unsafety.is_some() {
        return Err(syn::Error::new_spanned(
            function.sig.unsafety,
            "#[timeout] does not support unsafe functions",
        ));
    }
    if !returns_result(&function.sig.output) {
        return Err(syn::Error::new_spanned(
            &function.sig.output,
            "#[timeout] functions must return Result<T, E>",
        ));
    }

    let attrs = &function.attrs;
    let vis = &function.vis;
    let sig = &function.sig;
    let block = &function.block;
    let name = args.name;

    Ok(quote! {
        #(#attrs)*
        #vis #sig {
            use ::summer::plugin::ComponentRegistry as _;

            let __summer_timeout_registry = ::summer::App::global()
                .get_expect_component::<::summer_resilience::TimeoutRegistry>();
            let __summer_timeout_policy = __summer_timeout_registry.get(#name)
                .unwrap_or_else(|| panic!("timeout policy `{}` is not configured", #name));

            match ::summer_resilience::timeout::execute(
                #name,
                __summer_timeout_policy,
                async move #block,
            )
            .await
            {
                ::core::result::Result::Ok(value) => ::core::result::Result::Ok(value),
                ::core::result::Result::Err(
                    ::summer_resilience::TimeoutError::Operation(error),
                ) => ::core::result::Result::Err(error),
                ::core::result::Result::Err(
                    ::summer_resilience::TimeoutError::Elapsed(error),
                ) => ::core::result::Result::Err(::core::convert::From::from(error)),
            }
        }
    })
}

fn returns_result(output: &ReturnType) -> bool {
    let ReturnType::Type(_, ty) = output else {
        return false;
    };
    let Type::Path(type_path) = ty.as_ref() else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Result")
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    fn args() -> TimeoutArgs {
        TimeoutArgs {
            name: LitStr::new("test", proc_macro2::Span::call_site()),
        }
    }

    #[test]
    fn rejects_synchronous_functions() {
        let function: ItemFn = parse_quote! {
            fn operation() -> Result<(), ()> { Ok(()) }
        };
        assert!(expand(args(), function)
            .unwrap_err()
            .to_string()
            .contains("async"));
    }

    #[test]
    fn rejects_non_result_functions() {
        let function: ItemFn = parse_quote! {
            async fn operation() -> usize { 1 }
        };
        assert!(expand(args(), function)
            .unwrap_err()
            .to_string()
            .contains("Result"));
    }
}
