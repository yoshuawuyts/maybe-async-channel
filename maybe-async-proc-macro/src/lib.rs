#![feature(option_get_or_insert_default)]

use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::{
    parse, parse_macro_input, parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::Comma,
    visit_mut::{visit_expr_mut, VisitMut},
    ConstParam, Expr, GenericParam, Ident, Item, ItemFn, PathArguments, ReturnType, Stmt,
};

#[proc_macro_attribute]
pub fn maybe_async(_attr: TokenStream, item: TokenStream) -> TokenStream {
    match parse_macro_input!(item as Item) {
        Item::Fn(item) => maybe_async_fn(item),
        Item::Trait(item) => maybe_async_trait(item),
        item => quote!(compile_error!(
            #item,
            "`maybe_async` is only valid for functions and trait declarations"
        ))
        .into(),
    }
}

fn maybe_async_fn(mut item: ItemFn) -> TokenStream {
    if let Some(asyncness) = item.sig.asyncness {
        return quote_spanned! {asyncness.span => compile_error!(
            "maybe_async functions can't also be `async`"
        );}
        .into();
    }
    item.sig.generics.lt_token.get_or_insert_default();
    item.sig.generics.gt_token.get_or_insert_default();
    let mod_name = &item.sig.ident;
    let async_effect = parse_quote!(const ASYNC: bool);
    item.sig
        .generics
        .params
        .insert(0, GenericParam::Const(async_effect));
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
    item.sig.output = parse_quote!(-> <() as #mod_name::Helper<ASYNC>>::Ret);

    let body = parse_quote!({<() as #mod_name::Helper<ASYNC>>::act(#call_args)});

    let body = std::mem::replace(&mut *item.block, body);

    let (mut body, mut async_body) = split_async_if_expression(body);

    Asyncifyier.visit_expr_mut(&mut async_body);
    Syncifyier.visit_block_mut(&mut body);

    let expanded = quote! {
        #item
        pub mod #mod_name {
            use super::*;
            pub trait Helper<const ASYNC: bool> {
                type Ret;
                fn act(#args) -> Self::Ret;
            }

            impl Helper<true> for () {
                type Ret = impl std::future::Future<Output = #ret>;
                fn act(#args) -> Self::Ret {
                    #async_body
                }
            }

            impl Helper<false> for () {
                type Ret = #ret;
                fn act(#args) -> Self::Ret
                    #body
            }

            // Actually only an impl for `MaybeAsync<false>`, as there are only two possible impls
            // and we wrote both of them. Workaround for https://github.com/rust-lang/rust/pull/104803
            impl<const B: bool> Helper<B> for () {
                default type Ret = ();
                #[allow(unused_variables)]
                default fn act(#args) -> Self::Ret {
                    panic!("your trait solver is broken")
                }
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
                        let args: TokenStream = quote!(::<true>).into();
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
                        let args: TokenStream = quote!(::<false>).into();
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

fn maybe_async_trait(mut item: syn::ItemTrait) -> TokenStream {
    item.generics.lt_token.get_or_insert_default();
    item.generics.gt_token.get_or_insert_default();
    let async_effect: ConstParam = parse_quote!(const ASYNC: bool);
    item.generics
        .params
        .insert(0, GenericParam::Const(async_effect.clone()));

    let mut new_items = vec![];

    for assoc in &mut item.items {
        match assoc {
            syn::TraitItem::Method(method) => {
                // FIXME: use `drain_filter` when that becomes stable
                let pos = method
                    .attrs
                    .iter()
                    .position(|attr| attr.path.is_ident("maybe_async"));
                if let Some(pos) = pos {
                    method.attrs.remove(pos);
                } else {
                    continue;
                }
                if let Some(asyncness) = method.sig.asyncness {
                    return quote_spanned! {asyncness.span => compile_error!(
                        "maybe_async methods can't also be `async`"
                    );}
                    .into();
                }
                method.sig.generics.lt_token.get_or_insert_default();
                method.sig.generics.gt_token.get_or_insert_default();
                let ret_name = Ident::new(
                    &format!("{}_ret", method.sig.ident),
                    method.sig.ident.span(),
                );
                method
                    .sig
                    .generics
                    .params
                    .insert(0, GenericParam::Lifetime(parse_quote!('a)));
                method.sig.output = parse_quote!(-> Self::#ret_name<'a>);
                if let Some(def) = &method.default {
                    return quote_spanned!(def.span() => compile_error!("cannot specify `async` methods with default bodies in `maybe_async` traits");).into();
                }
                let ret_ty = parse_quote! {
                    #[allow(non_camel_case_types)]
                    type #ret_name<'a> where Self: 'a;
                };
                new_items.push(syn::TraitItem::Type(ret_ty));

                if let Some(first) = method.sig.inputs.first_mut() {
                    if let syn::FnArg::Receiver(recv) = first {
                        if let Some((_, lifetime)) = &mut recv.reference {
                            if let Some(lifetime) = lifetime {
                                return quote_spanned!(lifetime.span() => compile_error!("`self` parameter already has a named lifetime");).into();
                            }
                            *lifetime = Some(parse_quote!('a));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    item.items.extend(new_items);

    quote! { #item }.into()
}
