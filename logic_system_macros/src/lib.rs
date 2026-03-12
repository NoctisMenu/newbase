use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    AngleBracketedGenericArguments, Error, FnArg, GenericArgument, GenericParam, Ident, ItemFn,
    LitStr, PathArguments, Result, ReturnType, Token, Type, TypePath, TypeReference, parse_macro_input,
};

struct LogicSystemArgs {
    name: Option<LitStr>,
}

impl Parse for LogicSystemArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut name = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            if key == "name" {
                if name.is_some() {
                    return Err(Error::new(key.span(), "duplicate `name` argument"));
                }
                name = Some(input.parse::<LitStr>()?);
            } else {
                return Err(Error::new(
                    key.span(),
                    "unknown argument; supported arguments: `name = \"...\"`",
                ));
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self { name })
    }
}

#[proc_macro_attribute]
pub fn logic_system(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as LogicSystemArgs);
    let input_fn = parse_macro_input!(item as ItemFn);

    match expand_logic_system(args, input_fn) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_logic_system(args: LogicSystemArgs, input_fn: ItemFn) -> Result<proc_macro2::TokenStream> {
    let sig = &input_fn.sig;
    let fn_ident = &sig.ident;

    if sig.asyncness.is_some() {
        return Err(Error::new(sig.fn_token.span, "logic system function cannot be async"));
    }
    if sig.constness.is_some() {
        return Err(Error::new(sig.fn_token.span, "logic system function cannot be const"));
    }
    if !matches!(sig.output, ReturnType::Default) {
        return Err(Error::new(
            sig.output.span(),
            "logic system function must return `()`",
        ));
    }

    if sig.inputs.len() != 3 {
        return Err(Error::new(
            sig.paren_token.span.open(),
            "logic system function must have exactly 3 arguments: app, ui, draw_list",
        ));
    }

    let has_s = sig
        .generics
        .params
        .iter()
        .any(|param| matches!(param, GenericParam::Type(tp) if tp.ident == "S"));

    let mut inputs = sig.inputs.iter();
    let app_arg = inputs.next().unwrap();
    let ui_arg = inputs.next().unwrap();
    let draw_list_arg = inputs.next().unwrap();

    let app_ty = extract_arg_type(app_arg, "first")?;
    let ui_ty = extract_arg_type(ui_arg, "second")?;
    let draw_list_ty = extract_arg_type(draw_list_arg, "third")?;

    let system_ident = function_name_to_system_ident(fn_ident)?;
    let display_name = args
        .name
        .unwrap_or_else(|| LitStr::new(&fn_ident.to_string(), fn_ident.span()));

    let generics = &sig.generics;
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let impl_block = if has_s {
        quote! {
            impl #impl_generics LogicSystem<S> for #system_ident #where_clause {
                fn name(&self) -> &'static str {
                    #display_name
                }

                fn tick(
                    &mut self,
                    app: #app_ty,
                    ui: #ui_ty,
                    draw_list: #draw_list_ty,
                ) {
                    #fn_ident(app, ui, draw_list);
                }
            }
        }
    } else {
        if !sig.generics.params.is_empty() {
            return Err(Error::new(
                sig.generics.span(),
                "when `S` is omitted, logic system function must not declare other generics",
            ));
        }

        let state_ty = infer_state_ty_from_app_arg(&app_ty)?;
        quote! {
            impl LogicSystem<#state_ty> for #system_ident {
                fn name(&self) -> &'static str {
                    #display_name
                }

                fn tick(
                    &self,
                    app: #app_ty,
                    ui: #ui_ty,
                    draw_list: #draw_list_ty,
                ) {
                    #fn_ident(app, ui, draw_list);
                }
            }
        }
    };

    Ok(quote! {
        #input_fn

        struct #system_ident;

        #impl_block
    })
}

fn extract_arg_type(arg: &FnArg, label: &str) -> Result<syn::Type> {
    match arg {
        FnArg::Receiver(receiver) => Err(Error::new(
            receiver.span(),
            format!("logic system {} argument cannot be `self`", label),
        )),
        FnArg::Typed(typed) => Ok((*typed.ty).clone()),
    }
}

fn function_name_to_system_ident(fn_ident: &Ident) -> Result<Ident> {
    let fn_name = fn_ident.to_string();
    let mut pascal = String::new();
    let mut uppercase_next = true;

    for ch in fn_name.chars() {
        if ch == '_' || !ch.is_ascii_alphanumeric() {
            uppercase_next = true;
            continue;
        }

        if uppercase_next {
            pascal.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            pascal.push(ch);
        }
    }

    if pascal.is_empty() {
        return Err(Error::new(
            fn_ident.span(),
            "cannot derive system type name from function name",
        ));
    }

    Ok(format_ident!("{}", pascal, span = fn_ident.span()))
}

fn infer_state_ty_from_app_arg(app_ty: &Type) -> Result<Type> {
    let Type::Reference(TypeReference { elem, .. }) = app_ty else {
        return Err(Error::new(
            app_ty.span(),
            "first argument must be `&mut App<S>` or `&mut App<ConcreteState>`",
        ));
    };

    let Type::Path(TypePath { path, .. }) = elem.as_ref() else {
        return Err(Error::new(
            elem.span(),
            "first argument must reference `App<...>`",
        ));
    };

    let Some(last) = path.segments.last() else {
        return Err(Error::new(path.span(), "invalid app type path"));
    };

    if last.ident != "App" {
        return Err(Error::new(
            last.ident.span(),
            "first argument must reference `App<...>`",
        ));
    }

    let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) = &last.arguments
    else {
        return Err(Error::new(
            last.arguments.span(),
            "App argument must include a concrete state type, e.g. `App<AppData>`",
        ));
    };

    if args.len() != 1 {
        return Err(Error::new(
            args.span(),
            "App must have exactly one generic parameter",
        ));
    }

    let Some(GenericArgument::Type(state_ty)) = args.first() else {
        return Err(Error::new(
            args.span(),
            "App generic argument must be a type",
        ));
    };

    Ok(state_ty.clone())
}
