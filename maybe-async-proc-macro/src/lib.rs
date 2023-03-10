#![feature(option_get_or_insert_default)]

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, quote_spanned, ToTokens};
use syn::{
    parse,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::Comma,
    visit_mut::{visit_expr_mut, VisitMut},
    ConstParam, Error, Expr, GenericParam, Ident, Item, ItemFn, PathArguments, ReturnType, Stmt,
    Token,
};

#[derive(Debug, Eq)]
enum KeywordKind {
    Async,
    Try,
}

impl PartialEq for KeywordKind {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

#[derive(Debug)]
struct Keyword {
    span: Span,
    kind: KeywordKind,
}

impl Eq for Keyword {}
impl PartialEq for Keyword {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Keyword {
    fn identify(&self, name: &str) -> Ident {
        Ident::new(name, self.span)
    }

    fn all_caps_name(&self) -> &'static str {
        match self.kind {
            KeywordKind::Async => "ASYNC",
            KeywordKind::Try { .. } => "TRY",
        }
    }
}

impl Parse for Keyword {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![async]) {
            let tok = input.parse::<Token![async]>()?;
            Ok(Self {
                span: tok.span,
                kind: KeywordKind::Async,
            })
        } else if lookahead.peek(Token![try]) {
            let tok = input.parse::<Token![try]>()?;
            Ok(Self {
                span: tok.span,
                kind: KeywordKind::Try,
            })
        } else {
            Err(Error::new(
                input.span(),
                "unknown keyword, expected `async` or `try`",
            ))
        }
    }
}

struct MyMacroInput {
    keywords: Vec<Keyword>,
    span: Span,
}

impl ToTokens for MyMacroInput {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let keywords = self.keywords.iter();
        quote_spanned!(self.span => #(#keywords),*).to_tokens(tokens)
    }
}

impl ToTokens for Keyword {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self.kind {
            KeywordKind::Async => quote_spanned!(self.span => async),
            KeywordKind::Try { .. } => quote_spanned!(self.span => try),
        }
        .to_tokens(tokens)
    }
}

impl Parse for MyMacroInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();
        let mut keywords = Vec::new();
        loop {
            let kw: Keyword = input.parse()?;
            if keywords.contains(&kw) {
                return Err(Error::new(input.span(), "duplicate keyword"));
            }
            keywords.push(kw);
            if input.is_empty() {
                return Ok(Self { keywords, span });
            }
            input.parse::<Token![,]>()?;
        }
    }
}

#[proc_macro_attribute]
pub fn maybe(attr: TokenStream, item: TokenStream) -> TokenStream {
    let kinds = parse_macro_input!(attr as MyMacroInput);
    if kinds.keywords.len() != 1 {
        return quote!(compile_error!(
            #kinds,
            "`maybe` currently only supports exactly one keyword"
        );)
        .into();
    }
    match parse_macro_input!(item as Item) {
        Item::Fn(item) => maybe_fn(item, kinds.keywords),
        Item::Trait(item) => maybe_async_trait(item),
        item => quote!(compile_error!(
            #item,
            "`maybe` is only valid for functions and trait declarations"
        );)
        .into(),
    }
}

fn maybe_fn(mut item: ItemFn, kinds: Vec<Keyword>) -> TokenStream {
    if let Some(asyncness) = item.sig.asyncness {
        return quote_spanned! {asyncness.span => compile_error!(
            "maybe_async functions can't also be `async`"
        );}
        .into();
    }
    item.sig.generics.lt_token.get_or_insert_default();
    item.sig.generics.gt_token.get_or_insert_default();
    let mod_name = &item.sig.ident;
    let effect_param = parse_quote!(const EFFECT: maybe_async_std::prelude::Effects);
    item.sig
        .generics
        .params
        .push(GenericParam::Const(effect_param));
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
    item.sig.output = parse_quote!(-> <() as #mod_name::Helper<EFFECT>>::Ret);

    let body = parse_quote!({<() as #mod_name::Helper<EFFECT>>::act(#call_args)});

    let body = std::mem::replace(&mut *item.block, body);

    let (mut body, effect_bodies) = split_if_expression(body, &kinds);

    let effect_bodies = kinds.iter().zip(effect_bodies).map(|(effect, mut body)| {
        Effectifier(&effect, &kinds).visit_expr_mut(&mut body);
        let effect_name = effect.identify(effect.all_caps_name());
        let ret = match &effect.kind {
            KeywordKind::Async => quote!(impl std::future::Future<Output = #ret>),
            KeywordKind::Try => quote!(#ret),
        };
        let effect = quote!(Effects::#effect_name);
        quote! {
            impl Helper<{#effect}> for () {
                type Ret = #ret;
                fn act(#args) -> Self::Ret {
                    #body
                }
            }
        }
    });
    DeEffectifier(&kinds).visit_block_mut(&mut body);

    assert_eq!(kinds.len(), 1);
    let ret = match kinds[0].kind {
        KeywordKind::Async => quote!(#ret),
        KeywordKind::Try => quote!(<#ret as std::ops::Try>::Output),
    };

    let expanded = quote! {
        #item
        pub mod #mod_name {
            use super::*;
            use maybe_async_std::prelude::Effects;
            pub trait Helper<const EFFECT: Effects> {
                type Ret;
                fn act(#args) -> Self::Ret;
            }

            #(#effect_bodies)*

            impl Helper<{Effects::NONE}> for () {
                type Ret = #ret;
                fn act(#args) -> Self::Ret
                    #body
            }

            // Actually only an impl for `MaybeAsync<NONE>`, as there are only two possible impls
            // and we wrote both of them. Workaround for https://github.com/rust-lang/rust/pull/104803
            impl<const EFFECT: Effects> Helper<EFFECT> for () {
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

fn split_if_expression(body: syn::Block, effects: &[Keyword]) -> (syn::Block, Vec<syn::Expr>) {
    let per_effect = |body| -> Vec<syn::Expr> {
        effects
            .iter()
            .map(|effect| match effect.kind {
                KeywordKind::Async => parse_quote!(async move #body),
                KeywordKind::Try { .. } => parse_quote!(try #body),
            })
            .collect()
    };
    if let [Stmt::Expr(Expr::If(expr_if))] = &body.stmts[..] {
        if let Expr::Path(path) = &*expr_if.cond {
            if path.qself.is_none() {
                if let Some(kw) = effects
                    .iter()
                    .find(|kw| path.path.is_ident(kw.all_caps_name()))
                {
                    let Expr::Block(sync) = &*expr_if.else_branch.as_ref().unwrap().1 else {
                        panic!()
                    };
                    let effect_expr = expr_if.then_branch.clone();
                    let mut bodies = per_effect(sync.block.clone());
                    for (effect, body) in effects.iter().zip(bodies.iter_mut()) {
                        if effect == kw {
                            *body = parse_quote!(#effect_expr);
                        }
                    }
                    return (sync.block.clone(), bodies);
                }
            }
        }
    }
    (body.clone(), per_effect(body))
}

struct Effectifier<'a>(&'a Keyword, &'a [Keyword]);

impl VisitMut for Effectifier<'_> {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        let muta = |expr: &mut _, kind: Ident| {
            if let Expr::Call(call) = expr {
                if let Expr::Path(path) = &mut *call.func {
                    let last = path.path.segments.last_mut().unwrap();
                    if let PathArguments::None = last.arguments {
                        let args: TokenStream =
                            quote!(::<{maybe_async_std::prelude::Effects::#kind}>).into();
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
        };
        for kw in self.1 {
            let kind = kw.identify(kw.all_caps_name());
            match kw.kind {
                KeywordKind::Async => {
                    if let Expr::Await(inner) = e {
                        muta(&mut *inner.base, kind);
                    }
                }
                KeywordKind::Try { .. } => {
                    if let Expr::Try(inner) = e {
                        muta(&mut *inner.expr, kind)
                    }
                }
            }
        }
        visit_expr_mut(self, e)
    }
}

struct DeEffectifier<'a>(&'a [Keyword]);

impl VisitMut for DeEffectifier<'_> {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        let muta = |mut inner| {
            if let Expr::Call(call) = &mut inner {
                if let Expr::Path(path) = &mut *call.func {
                    let last = path.path.segments.last_mut().unwrap();
                    if let PathArguments::None = last.arguments {
                        let args: TokenStream =
                            quote!(::<{maybe_async_std::prelude::Effects::NONE}>).into();
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
            inner
        };
        for kw in self.0 {
            match kw.kind {
                KeywordKind::Async => {
                    if let Expr::Await(inner) = e {
                        *e = muta((*inner.base).clone());
                    }
                }
                KeywordKind::Try { .. } => {
                    if let Expr::Try(inner) = e {
                        *e = muta((*inner.expr).clone());
                    }
                }
            }
        }
    }
}

fn maybe_async_trait(mut item: syn::ItemTrait) -> TokenStream {
    item.generics.lt_token.get_or_insert_default();
    item.generics.gt_token.get_or_insert_default();
    let async_effect: ConstParam = parse_quote!(const EFFECT: bool = false);
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
                    .position(|attr| attr.path.is_ident("maybe"));
                if let Some(pos) = pos {
                    let attr = method.attrs.remove(pos);
                    let input: MyMacroInput = attr.parse_args().unwrap();
                    assert_eq!(input.keywords.len(), 1, "only supporting async for now");
                    assert_eq!(
                        input.keywords.iter().next().unwrap().kind,
                        KeywordKind::Async,
                        "only supporting async for now"
                    );
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

                let ret_type =
                    std::mem::replace(&mut method.sig.output, parse_quote!(-> Self::#ret_name<'a>));
                if let Some(def) = &method.default {
                    return quote_spanned!(def.span() => compile_error!("cannot specify `async` methods with default bodies in `maybe_async` traits");).into();
                }
                let ret_type = match ret_type {
                    ReturnType::Default => parse_quote!(()),
                    ReturnType::Type(_, ty) => *ty,
                };
                let ret_ty = parse_quote! {
                    #[allow(non_camel_case_types)]
                    type #ret_name<'a> = #ret_type where Self: 'a;
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
