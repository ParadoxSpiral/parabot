// Copyright (C) 2018  ParadoxSpiral
//
// This file is part of parabot.
//
// Parabot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Parabot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Parabot.  If not, see <http://www.gnu.org/licenses/>.

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    self,
    parse::{Error, Parse, ParseStream, Result},
    parse_macro_input,
    spanned::Spanned,
    FnArg, ItemFn, ItemStruct, LitStr, Path, PathSegment, Token, Type, TypePath, TypeReference,
};

#[proc_macro_attribute]
pub fn module(args: TokenStream, input: TokenStream) -> TokenStream {
    // Check if we are called on a struct, or fn definition
    if let Ok(parsed) = syn::parse::<ItemStruct>(input.clone()) {
        build_struct(args, parsed)
    } else if let Ok(parsed) = syn::parse::<ItemFn>(input) {
        build_fn(args, parsed)
    } else {
        panic!("This macro only operates on struct and funtion definitions")
    }
}

struct StructAttrs {
    help: LitStr,
    handles: TokenStream2,
    impls: TokenStream2,
}
impl Parse for StructAttrs {
    fn parse(input: ParseStream) -> Result<Self> {
        let id: Ident = input.parse()?;
        let help = if id == "help" && input.peek(Token![=]) {
            input.parse::<Token![=]>().unwrap();
            input.parse::<LitStr>()?
        } else {
            return Err(Error::new(id.span(), "expected `help = \"…\"`"));
        };
        input
            .parse::<Token![,]>()
            .map_err(|_| input.error("expected at least one handler"))?;

        let mut impls = TokenStream2::new();
        let mut handles = TokenStream2::new();
        for id in input.parse_terminated::<_, Token![,]>(Ident::parse)?.iter() {
            match &*id.to_string() {
                "connected" => {
                    handles.extend(quote! {
                        Stage::Connected => true,
                    });
                    impls.extend(quote! {
                        fn connected(&mut self, client: &Arc<IrcClient>, mctx: &MessageContext,
                                     conn: &DbConn, cfg: &mut ModuleCfg) {
                            self.impl_detail_handle_connected(client, mctx, conn, cfg)
                        }
                    });
                }
                "received" => {
                    handles.extend(quote! {
                        Stage::Received => true,
                    });
                    impls.extend(quote! {
                        fn received(&mut self, client: &Arc<IrcClient>, mctx: &MessageContext,
                                    conn: &DbConn, cfg: &mut ModuleCfg, msg: &Message,
                                    trigger: Trigger) {
                            self.impl_detail_handle_received(client, mctx, conn, cfg, msg, trigger)
                        }
                    });
                }
                "pre_send" => {
                    handles.extend(quote! {
                        Stage::PreSend => true,
                    });
                    impls.extend(quote! {
                        fn pre_send(&mut self, client: &Arc<IrcClient>, mctx: &MessageContext,
                                    conn: &DbConn, cfg: &mut ModuleCfg, msg: &Message) {
                            self.impl_detail_handle_pre_send(client, mctx, conn, cfg, msg)
                        }
                    });
                }
                "post_send" => {
                    handles.extend(quote! {
                        Stage::PostSend => true,
                    });
                    impls.extend(quote! {
                        fn post_send(&mut self, client: &Arc<IrcClient>, mctx: &MessageContext,
                                    conn: &DbConn, cfg: &mut ModuleCfg, msg: &Message) {
                            self.impl_detail_handle_post_send(client, mctx, conn, cfg, msg)
                        }
                    });
                }
                _ => {
                    return Err(
                        input.error("expected one of: connected, received, pre_send, post_send")
                    );
                }
            }
        }
        if handles.is_empty() {
            return Err(input.error("expected at least one handler"));
        }

        Ok(Self {
            help,
            handles,
            impls,
        })
    }
}

fn build_struct(args: TokenStream, parsed: ItemStruct) -> TokenStream {
    let attrs = parse_macro_input!(args as StructAttrs);

    let name = &parsed.ident;
    let help = attrs.help;
    let handles = attrs.handles;
    let impls = attrs.impls;
    let gen = quote! {
        #parsed

        impl Module for #name {
            fn help(&self) -> &'static str {
                #help
            }

            fn handles(&self, stage: Stage) -> bool {
                match stage {
                    #handles
                    _ => false,
                }
            }

            #impls
        }
    };
    gen.into()
}

struct FnAttrs {
    impl_target: Ident,
    fn_decl: TokenStream2,
}
impl Parse for FnAttrs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut iter = input
            .parse_terminated::<_, Token![,]>(Ident::parse)?
            .into_iter();
        let impl_target = iter
            .next()
            .ok_or_else(|| input.error("expected module ident"))?;
        let fn_decl = match &*iter
            .next()
            .ok_or_else(|| input.error("expected handler kind"))?
            .to_string()
        {
            "connected" => quote! {
                    impl_detail_handle_connected(&mut self,
                                                 client: &Arc<IrcClient>,
                                                 mctx: &MessageContext,
                                                 conn: &DbConn,
                                                 cfg: &mut ModuleCfg)
            },
            "received" => quote! {
                    impl_detail_handle_received(&mut self,
                                                client: &Arc<IrcClient>,
                                                mctx: &MessageContext,
                                                conn: &DbConn,
                                                cfg: &mut ModuleCfg,
                                                msg: &Message,
                                                trigger: Trigger)
            },
            "pre_send" => quote! {
                    impl_detail_handle_pre_send(&mut self,
                                                client: &Arc<IrcClient>,
                                                mctx: &MessageContext,
                                                conn: &DbConn,
                                                cfg: &mut ModuleCfg,
                                                msg: &Message)
            },
            "post_send" => quote! {
                    impl_detail_handle_post_send(&mut self,
                                                 client: &Arc<IrcClient>,
                                                 mctx: &MessageContext,
                                                 conn: &DbConn,
                                                 cfg: &mut ModuleCfg,
                                                 msg: &Message)
            },
            _ => {
                return Err(input.error("expected one of: connected, received, pre_send, post_send"));
            }
        };

        Ok(Self {
            impl_target,
            fn_decl,
        })
    }
}

fn build_fn(args: TokenStream, parsed: ItemFn) -> TokenStream {
    let attrs = parse_macro_input!(args as FnAttrs);
    let target = attrs.impl_target;

    let mut args = TokenStream2::new();
    let mut iter = parsed.decl.inputs.iter();
    while let Some(FnArg::Captured(arg)) = iter.next() {
        match &arg.ty {
            Type::Reference(TypeReference { elem, .. }) => match &**elem {
                Type::Path(TypePath {
                    path: Path { segments, .. },
                    ..
                }) => {
                    let PathSegment { ref ident, .. } = segments[0];
                    match &*ident.to_string() {
                        ty if ty == &target.to_string() => {
                            args.extend(quote! { self, });
                        }
                        "Arc" => {
                            args.extend(quote! { client, });
                        }
                        "MessageContext" => {
                            args.extend(quote! { mctx, });
                        }
                        "DbConn" => {
                            args.extend(quote! { conn, });
                        }
                        "ModuleCfg" => {
                            args.extend(quote! { cfg, });
                        }
                        "Message" => {
                            args.extend(quote! { msg, });
                        }
                        _ => {
                            args.extend(
                                Error::new(
                                    arg.ty.span(),
                                    "expected one type of: \
                                     `&Arc<IrcClient>`, `&MessageContext`, `&DbConn`, \
                                     `&mut ModuleCfg`, `&Message`, `Trigger`",
                                )
                                .to_compile_error(),
                            );
                        }
                    }
                }
                _ => {
                    args.extend(
                        Error::new(
                            arg.ty.span(),
                            "expected one type of: \
                             `&Arc<IrcClient>`, `&MessageContext`, `&DbConn`, \
                             `&mut ModuleCfg`, `&Message`, `Trigger`",
                        )
                        .to_compile_error(),
                    );
                }
            },
            Type::Path(TypePath {
                path: Path { segments, .. },
                ..
            }) => {
                let PathSegment { ref ident, .. } = segments[0];
                match &*ident.to_string() {
                    "Trigger" => {
                        args.extend(quote! { trigger, });
                    }
                    _ => {
                        args.extend(
                            Error::new(
                                arg.ty.span(),
                                "expected one type of: \
                                 `&Arc<IrcClient>`, `&MessageContext`, `&mut ModuleCfg`, \
                                 `&Message`, `Trigger`",
                            )
                            .to_compile_error(),
                        );
                    }
                }
            }
            _ => {
                args.extend(
                    Error::new(
                        arg.ty.span(),
                        "expected one type of: \
                         `&Arc<IrcClient>`, `&MessageContext`, `&mut ModuleCfg`, \
                         `&Message`, `Trigger`",
                    )
                    .to_compile_error(),
                );
            }
        }
    }

    let fun = &parsed.ident;
    let fn_decl = attrs.fn_decl;
    let gen = quote! {
        #parsed

        impl #target {
            fn #fn_decl {
                #fun(#args)
            }
        }
    };

    gen.into()
}
