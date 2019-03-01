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

use darling::{util::SpannedValue, FromMeta};
use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    self, parse::Error, parse_macro_input, spanned::Spanned, AttributeArgs, FnArg, GenericArgument,
    ItemFn, ItemStruct, Path, PathArguments, PathSegment, Type, TypePath, TypeReference,
};

#[proc_macro_attribute]
pub fn module(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as AttributeArgs);
    if let Ok(parsed) = syn::parse::<ItemStruct>(input.clone()) {
        build_struct(args, parsed)
    } else if let Ok(parsed) = syn::parse::<ItemFn>(input) {
        build_fn(args, parsed)
    } else {
        panic!("This macro only operates on struct and funtion definitions")
    }
}

#[derive(Debug, FromMeta)]
enum Handler {
    Connected,
    Received,
    #[darling(rename = "pre_send")]
    PreSend,
    #[darling(rename = "post_send")]
    PostSend,
}

#[derive(Debug, FromMeta)]
struct StructArgs {
    help: String,
    #[darling(multiple)]
    handles: Vec<Handler>,
}

fn build_struct(args: AttributeArgs, parsed: ItemStruct) -> TokenStream {
    let args = SpannedValue::new(
        StructArgs::from_list(&args).unwrap(),
        args.first().unwrap().span(),
    );

    let name = &parsed.ident;
    let help = &args.help;

    if args.handles.is_empty() {
        return Error::new(
            args.span(),
            "expected at least one handler: connected, received, pre_send, post_send",
        )
        .to_compile_error()
        .into();
    }

    let mut impls = TokenStream2::new();
    let mut handles = TokenStream2::new();
    for handler in &args.handles {
        match handler {
            Handler::Connected => {
                handles.extend(quote! { Stage::Connected => true, });
                impls.extend(quote! {
                    fn connected(&mut self, client: &Arc<IrcClient>, mctx: &Arc<MessageContext>,
                                 conn: &DbConn, cfg: &mut ModuleCfg) {
                        self.impl_handle_connected(client, mctx, conn, cfg)
                    }
                });
            }
            Handler::Received => {
                handles.extend(quote! { Stage::Received => true, });
                impls.extend(quote! {
                    fn received(&mut self, client: &Arc<IrcClient>, mctx: &Arc<MessageContext>,
                                conn: &DbConn, cfg: &mut ModuleCfg, msg: &Message,
                                trigger: Trigger) {
                        self.impl_handle_received(client, mctx, conn, cfg, msg, trigger)
                    }
                });
            }
            Handler::PreSend => {
                handles.extend(quote! { Stage::PreSend => true, });
                impls.extend(quote! {
                    fn pre_send(&mut self, client: &Arc<IrcClient>, mctx: &Arc<MessageContext>,
                                conn: &DbConn, cfg: &mut ModuleCfg, msg: &Message) {
                        self.impl_handle_pre_send(client, mctx, conn, cfg, msg)
                    }
                });
            }
            Handler::PostSend => {
                handles.extend(quote! { Stage::PostSend => true, });
                impls.extend(quote! {
                    fn post_send(&mut self, client: &Arc<IrcClient>, mctx: &Arc<MessageContext>,
                                conn: &DbConn, cfg: &mut ModuleCfg, msg: &Message) {
                        self.impl_handle_post_send(client, mctx, conn, cfg, msg)
                    }
                });
            }
        }
    }

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

#[derive(Debug, FromMeta)]
struct FnArgs {
    // TODO: Use Path to support non-local impls
    #[darling(rename = "belongs_to")]
    impl_target: Ident,
    handles: Handler,
}

fn build_fn(args: AttributeArgs, parsed: ItemFn) -> TokenStream {
    let args = SpannedValue::new(
        FnArgs::from_list(&args).unwrap(),
        args.first().unwrap().span(),
    );

    let fun = &parsed.ident;
    let fun_decl = match args.handles {
        Handler::Connected => quote! {
            impl_handle_connected(&mut self, client: &Arc<IrcClient>, mctx: &Arc<MessageContext>,
                                  conn: &DbConn, cfg: &mut ModuleCfg)
        },
        Handler::Received => quote! {
            impl_handle_received(&mut self, client: &Arc<IrcClient>, mctx: &Arc<MessageContext>,
                                 conn: &DbConn, cfg: &mut ModuleCfg, msg: &Message, trigger: Trigger)
        },
        Handler::PreSend => quote! {
            impl_handle_pre_send(&mut self, client: &Arc<IrcClient>, mctx: &Arc<MessageContext>,
                                 conn: &DbConn, cfg: &mut ModuleCfg, msg: &Message)
        },
        Handler::PostSend => quote! {
            impl_handle_post_send(&mut self, client: &Arc<IrcClient>, mctx: &Arc<MessageContext>,
                                  conn: &DbConn, cfg: &mut ModuleCfg, msg: &Message)
        },
    };

    let impl_target = &args.impl_target;
    let mut impl_args = TokenStream2::new();
    let mut iter = parsed.decl.inputs.iter();
    let err = |span| {
        Error::new(
            span,
            "expected one type of: `&Arc<IrcClient>`, `&Arc<MessageContext>`, `&DbConn`, \
             `&mut ModuleCfg`, `&Message`, `Trigger`",
        )
        .to_compile_error()
    };

    while let Some(FnArg::Captured(arg)) = iter.next() {
        match &arg.ty {
            Type::Reference(TypeReference { elem, .. }) => match &**elem {
                Type::Path(TypePath {
                    path: Path { segments, .. },
                    ..
                }) => {
                    let PathSegment {
                        ref ident,
                        ref arguments,
                    } = segments[0];
                    match &*ident.to_string() {
                        ty if ty == &impl_target.to_string() => {
                            impl_args.extend(quote! { self, });
                        }
                        "Arc" => {
                            let arguments =
                                if let PathArguments::AngleBracketed(arguments) = arguments {
                                    arguments
                                } else {
                                    impl_args.extend(err(arguments.span()));
                                    continue;
                                };
                            match &arguments.args[0] {
                                GenericArgument::Type(Type::Path(TypePath {
                                    path: Path { segments, .. },
                                    ..
                                })) => {
                                    let PathSegment { ref ident, .. } = segments[0];
                                    match &*ident.to_string() {
                                        "IrcClient" => {
                                            impl_args.extend(quote! { client, });
                                        }
                                        "MessageContext" => {
                                            impl_args.extend(quote! { mctx, });
                                        }
                                        _ => {
                                            impl_args.extend(err(ident.span()));
                                        }
                                    }
                                }
                                _ => {
                                    impl_args.extend(err(arg.ty.span()));
                                }
                            }
                        }
                        "DbConn" => {
                            impl_args.extend(quote! { conn, });
                        }
                        "ModuleCfg" => {
                            impl_args.extend(quote! { cfg, });
                        }
                        "Message" => {
                            impl_args.extend(quote! { msg, });
                        }
                        _ => {
                            impl_args.extend(err(ident.span()));
                        }
                    }
                }
                _ => {
                    impl_args.extend(err(arg.ty.span()));
                }
            },
            Type::Path(TypePath {
                path: Path { segments, .. },
                ..
            }) => {
                let PathSegment { ref ident, .. } = segments[0];
                match &*ident.to_string() {
                    "Trigger" => {
                        impl_args.extend(quote! { trigger, });
                    }
                    _ => {
                        impl_args.extend(err(ident.span()));
                    }
                }
            }
            _ => {
                impl_args.extend(err(arg.ty.span()));
            }
        }
    }

    let gen = quote! {
        #parsed

        impl #impl_target {
            fn #fun_decl {
                #fun(#impl_args)
            }
        }
    };

    gen.into()
}
