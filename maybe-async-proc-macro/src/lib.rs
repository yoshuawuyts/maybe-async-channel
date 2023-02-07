#![feature(option_get_or_insert_default)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse, parse_macro_input, parse_quote,
    punctuated::Punctuated,
    token::Comma,
    visit_mut::{visit_expr_mut, VisitMut},
    Expr, GenericParam, ItemFn, PathArguments, ReturnType, Stmt,
};

#[proc_macro_attribute]
pub fn maybe_async(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(item as ItemFn);
    assert!(item.sig.asyncness.is_some());
    item.sig.asyncness = None;
    item.sig.generics.lt_token.get_or_insert_default();
    item.sig.generics.gt_token.get_or_insert_default();
    let mod_name = &item.sig.ident;
    let async_effect = parse_quote!(ASYNC: #mod_name::Helper);
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
    let ret = match &item.sig.output {
        ReturnType::Default => quote!(()),
        ReturnType::Type(_, t) => quote!(#t),
    };
    item.sig.output = parse_quote!(-> <ASYNC as #mod_name::Helper>::Ret);

    let body = parse_quote!({<ASYNC as #mod_name::Helper>::act(#call_args)});

    let body = std::mem::replace(&mut *item.block, body);

    let (mut body, mut async_body) = split_async_if_expression(body);

    Asyncifyier.visit_expr_mut(&mut async_body);
    Syncifyier.visit_block_mut(&mut body);

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
                    #async_body
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

fn split_async_if_expression(body: syn::Block) -> (syn::Block, syn::Expr) {
    if let [Stmt::Expr(Expr::If(expr_if))] = &body.stmts[..] {
        if let Expr::Path(path) = &*expr_if.cond {
            if path.qself.is_none() && path.path.is_ident("ASYNC") {
                let Expr::Block(sync) = &*expr_if.else_branch.as_ref().unwrap().1 else {
                    panic!()
                };
                let async_expr = expr_if.then_branch.clone();
                return (sync.block.clone(), parse_quote!(#async_expr));
            }
        }
    }
    (body.clone(), parse_quote!(async move #body))
}

struct Asyncifyier;

impl VisitMut for Asyncifyier {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        if let Expr::Await(inner) = e {
            if let Expr::Call(call) = &mut *inner.base {
                if let Expr::Path(path) = &mut *call.func {
                    let last = path.path.segments.last_mut().unwrap();
                    if let PathArguments::None = last.arguments {
                        let args: TokenStream = quote!(::<Self>).into();
                        last.arguments = PathArguments::AngleBracketed(parse(args).unwrap());
                    } else {
                        unimplemented!()
                    }
                } else {
                    todo!("emit a compile_error! invocation here so that we inform the user that they can only use await on *function* call expressions in maybe async functions");
                }
            } else {
                todo!("emit a compile_error! invocation here so that we inform the user that they can only use await on call expressions in maybe async functions");
            }
        }
        visit_expr_mut(self, e)
    }
}

struct Syncifyier;

impl VisitMut for Syncifyier {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        if let Expr::Await(inner) = e {
            let mut inner = (*inner.base).clone();
            if let Expr::Call(call) = &mut inner {
                if let Expr::Path(path) = &mut *call.func {
                    let last = path.path.segments.last_mut().unwrap();
                    if let PathArguments::None = last.arguments {
                        let args: TokenStream = quote!(::<Self>).into();
                        last.arguments = PathArguments::AngleBracketed(parse(args).unwrap());
                    } else {
                        unimplemented!()
                    }
                } else {
                    todo!("emit a compile_error! invocation here so that we inform the user that they can only use await on *function* call expressions in maybe async functions");
                }
            } else {
                todo!("emit a compile_error! invocation here so that we inform the user that they can only use await on call expressions in maybe async functions");
            }
            *e = inner;
        }
    }
}
