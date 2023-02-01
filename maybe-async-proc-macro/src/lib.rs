#![feature(option_get_or_insert_default)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ConstParam, GenericParam, ItemFn};

#[proc_macro_attribute]
pub fn maybe_async(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(item as ItemFn);
    assert!(item.sig.asyncness.is_some());
    item.sig.asyncness = None;
    item.sig.generics.lt_token.get_or_insert_default();
    item.sig.generics.gt_token.get_or_insert_default();
    let bool_param = TokenStream::from(quote!(const ASYNC: bool));
    let bool_param = parse_macro_input!(bool_param as ConstParam);
    item.sig
        .generics
        .params
        .insert(0, GenericParam::Const(bool_param));
    let expanded = quote! {
        #item
    };
    TokenStream::from(expanded)
}
