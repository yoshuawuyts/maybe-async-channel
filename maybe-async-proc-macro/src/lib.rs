#![feature(option_get_or_insert_default)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, punctuated::Punctuated, token::Comma, Block, GenericParam, ItemFn,
    ReturnType, TypeParam,
};

#[proc_macro_attribute]
pub fn maybe_async(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(item as ItemFn);
    assert!(item.sig.asyncness.is_some());
    item.sig.asyncness = None;
    item.sig.generics.lt_token.get_or_insert_default();
    item.sig.generics.gt_token.get_or_insert_default();
    let mod_name = &item.sig.ident;
    let async_effect = quote!(ASYNC: #mod_name::Helper).into();
    let async_effect = parse_macro_input!(async_effect as TypeParam);
    item.sig
        .generics
        .params
        .insert(0, GenericParam::Type(async_effect));
    let args = &item.sig.inputs;
    let call_args: Punctuated<_, Comma> = args
        .iter()
        .map(|arg| match arg {
            syn::FnArg::Receiver(r) => {
                let name = r.self_token;
                quote!(#name)
            }
            syn::FnArg::Typed(pt) => match &*pt.pat {
                syn::Pat::Ident(id) => {
                    let name = &id.ident;
                    quote!(#name)
                }
                _ => todo!(),
            },
        })
        .collect();
    let body = quote!({<ASYNC as #mod_name::Helper>::act(#call_args)}).into();
    let body = parse_macro_input!(body as Block);

    let body = std::mem::replace(&mut *item.block, body);
    let ret = match &item.sig.output {
        ReturnType::Default => quote!(()),
        ReturnType::Type(_, t) => quote!(#t),
    };
    let output = quote!(-> <ASYNC as #mod_name::Helper>::Ret).into();
    item.sig.output = parse_macro_input!(output as ReturnType);
    let expanded = quote! {
        #item
        pub mod #mod_name {
            use super::*;
            pub trait Helper {
                type Ret;
                fn act(#args) -> Self::Ret;
            }

            impl Helper for maybe_async_std::Async {
                type Ret = impl std::future::Future<Output = #ret>;
                fn act(#args) -> Self::Ret {
                    async move #body
                }
            }
            impl Helper for maybe_async_std::NotAsync {
                type Ret = #ret;
                fn act(#args) -> Self::Ret
                    #body
            }
        }
    };
    TokenStream::from(expanded)
}
