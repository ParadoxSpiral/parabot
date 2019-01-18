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
use quote::quote;
use syn::{self, parse_macro_input, DeriveInput, Meta, MetaList, MetaNameValue, NestedMeta};

// TODO: Nice error messages
#[proc_macro_derive(Module, attributes(module))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let attrs = if let Some(Ok(Meta::List(MetaList { nested: attrs, .. }))) =
        input.attrs.get(0).map(|a| a.parse_meta())
    {
        attrs
    } else {
        panic!("{} has malformed attributes", name);
    };

    let mut has_help = false;
    let mut impls = proc_macro2::TokenStream::new();
    let mut handles = proc_macro2::TokenStream::new();
    for attr in attrs {
        match attr {
            NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                ref ident, ref lit, ..
            })) if ident == "help" => {
                has_help = true;
                impls.extend(quote! {
                    fn help(&self) -> &'static str {
                        #lit
                    }
                });
            }
            NestedMeta::Meta(Meta::List(MetaList { ident, nested, .. })) => {
                let fun = if let Some(NestedMeta::Meta(Meta::Word(fun))) =
                    nested.first().as_ref().map(|p| p.value())
                {
                    fun
                } else {
                    panic!("{} message handler it not a valid ident", ident);
                };

                if nested.len() > 1 {
                    panic!("Only one message handler per stage per module supported");
                }

                if ident == "connected" {
                    handles.extend(quote! {
                        Stage::Connected => true,
                    });
                    impls.extend(quote!{
                        fn connected(&mut self, client: &Arc<IrcClient>, mctx: &MessageContext, cfg: &mut ModuleCfg) {
                            self.#fun(client, mctx, cfg)
                        }
                    });
                } else if ident == "received" {
                    handles.extend(quote! {
                        Stage::Received => true,
                    });
                    impls.extend(quote!{
                        fn received(&mut self, client: &Arc<IrcClient>, mctx: &MessageContext, cfg: &mut ModuleCfg, msg: &Message, trigger: Trigger) {
                            self.#fun(client, mctx, cfg, msg, trigger)
                        }
                    });
                } else if ident == "pre_send" {
                    handles.extend(quote! {
                        Stage::PreSend => true,
                    });
                    impls.extend(quote!{
                        fn pre_send(&mut self, client: &Arc<IrcClient>, mctx: &MessageContext, cfg: &mut ModuleCfg, msg: &Message) {
                            self.#fun(client, mctx, cfg, msg)
                        }
                    });
                } else if ident == "post_send" {
                    handles.extend(quote! {
                        Stage::PostSend => true,
                    });
                    impls.extend(quote!{
                        fn post_send(&mut self, client: &Arc<IrcClient>, mctx: &MessageContext, cfg: &mut ModuleCfg, msg: &Message) {
                            self.#fun(client, mctx, cfg, msg)
                        }
                    });
                } else {
                    panic!("{} is not a valid stage, expected one of `connected`, `received`, `pre_send`, `post_send`", ident);
                }
            }
            _ => panic!("{:#?}", attr),
        }
    }

    if !has_help {
        panic!("{} provides no help message", name);
    }
    if handles.is_empty() {
        panic!("{} provides no handler", name);
    }

    let gen = quote! {
        impl Module for #name {
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
