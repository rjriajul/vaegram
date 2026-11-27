//! The `BotCommands` impl written for an enum.

use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::{Data, DeriveInput, Field, Fields, Ident, Path, PathArguments, Type, Variant};

use crate::attrs::{EnumAttrs, VariantAttrs, field_rest};
use crate::rename::{fold, is_command_name, is_menu_name};

/// Most commands Telegram takes in one menu.
const MAX_MENU: usize = 100;

/// Shortest and longest description Telegram takes in the menu, in characters.
const MENU_DESCRIPTION: std::ops::RangeInclusive<usize> = 3..=256;

/// One variant, read and checked.
struct Command {
    ident: Ident,
    /// The command name first, then the aliases, each with where it was written.
    names: Vec<(String, Span)>,
    description: String,
    in_menu: bool,
    attrs: VariantAttrs,
    /// A block that evaluates to `Result<Self, CommandParseError>`, with the header in `__command`.
    parse: TokenStream,
}

/// Errors gathered so that one build reports all of them.
#[derive(Default)]
struct Errors(Option<syn::Error>);

impl Errors {
    fn push(&mut self, error: syn::Error) {
        match self.0.take() {
            Some(mut all) => {
                all.combine(error);
                self.0 = Some(all);
            }
            None => self.0 = Some(error),
        }
    }

    fn finish(self) -> syn::Result<()> {
        match self.0 {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

pub(crate) fn bot_commands(input: &DeriveInput) -> syn::Result<TokenStream> {
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new(
            input.ident.span(),
            "BotCommands derives only for an enum, with one variant per command",
        ));
    };
    if data.variants.is_empty() {
        return Err(syn::Error::new(input.ident.span(), "BotCommands needs at least one variant, one per command"));
    }
    let attrs = EnumAttrs::parse(&input.attrs)?;
    let mut errors = Errors::default();
    let mut commands = Vec::with_capacity(data.variants.len());
    for variant in &data.variants {
        match command(variant, &attrs) {
            Ok(command) => commands.push(command),
            Err(error) => errors.push(error),
        }
    }

    // Names are compared as `parse` and the filter compare them, so two that a user can't tell apart are refused.
    let mut seen: Vec<(String, &Ident)> = Vec::new();
    for command in &commands {
        for (name, span) in &command.names {
            let key = if attrs.case_sensitive { name.clone() } else { fold(name) };
            match seen.iter().find(|(seen_key, _)| *seen_key == key) {
                Some((_, owner)) => {
                    errors.push(syn::Error::new(*span, format!("`{name}` already names the command of `{owner}`")));
                }
                None => seen.push((key, &command.ident)),
            }
        }
    }
    let menu = commands.iter().filter(|c| c.in_menu).count();
    if menu > MAX_MENU {
        errors.push(syn::Error::new(
            input.ident.span(),
            format!(
                "Telegram's command menu takes at most {MAX_MENU} commands and this enum shows {menu}; keep some \
                 out of it with #[command(hide_menu)]"
            ),
        ));
    }
    errors.finish()?;
    Ok(generate(input, &attrs, &commands))
}

fn command(variant: &Variant, enum_attrs: &EnumAttrs) -> syn::Result<Command> {
    let attrs = VariantAttrs::parse(&variant.attrs)?;
    let ident = &variant.ident;
    let (name, name_span) = match &attrs.rename {
        Some(lit) => (lit.value(), lit.span()),
        None => (enum_attrs.rename_rule.apply(&ident.unraw().to_string()), ident.span()),
    };
    let mut names = vec![(name, name_span)];
    names.extend(attrs.aliases.iter().map(|lit| (lit.value(), lit.span())));

    let mut errors = Errors::default();
    for (name, span) in &names {
        if !is_command_name(name) {
            errors.push(syn::Error::new(
                *span,
                format!("`{name}` can't be a command: a command name is 1 to 32 letters, digits or underscores"),
            ));
        }
    }
    let in_menu = !(attrs.hide || attrs.hide_menu || enum_attrs.hide_menu);
    let description = attrs.description.clone().unwrap_or_default();
    if in_menu {
        let name = &names[0].0;
        if is_command_name(name) && !is_menu_name(name) {
            errors.push(syn::Error::new(
                name_span,
                format!(
                    "Telegram's command menu takes only lowercase English letters, digits and underscores, so \
                     `{name}` can't be in it; rename it, or keep it out of the menu with #[command(hide_menu)]"
                ),
            ));
        }
        let len = description.chars().count();
        if !MENU_DESCRIPTION.contains(&len) {
            errors.push(syn::Error::new(
                ident.span(),
                format!(
                    "`{name}` is in Telegram's command menu, which needs a description of 3 to 256 characters, and \
                     this one has {len}; write a doc comment or #[command(description = \"...\")], or keep it out \
                     of the menu with #[command(hide_menu)]"
                ),
            ));
        }
    }
    let parse = match parse_body(variant, &attrs, &enum_attrs.krate) {
        Ok(parse) => parse,
        Err(error) => {
            errors.push(error);
            TokenStream::new()
        }
    };
    errors.finish()?;
    Ok(Command { ident: ident.clone(), names, description, in_menu, attrs, parse })
}

/// The block that parses this variant's arguments out of `__command.rest()`.
fn parse_body(variant: &Variant, attrs: &VariantAttrs, krate: &Path) -> syn::Result<TokenStream> {
    let ident = &variant.ident;
    let fields: Vec<&Field> = variant.fields.iter().collect();
    if fields.is_empty() {
        if let Some(path) = &attrs.parse_with {
            return Err(syn::Error::new_spanned(path, "this command has no fields for `parse_with` to fill"));
        }
        let construct = match &variant.fields {
            Fields::Named(_) => quote!(Self::#ident {}),
            Fields::Unnamed(_) => quote!(Self::#ident()),
            Fields::Unit => quote!(Self::#ident),
        };
        let check = match attrs.allow_extra {
            Some(_) => TokenStream::new(),
            None => quote!(#krate::commands::split_args(__command.rest(), 0, 0)?;),
        };
        return Ok(quote!({ #check ::core::result::Result::Ok(#construct) }));
    }
    if let Some(span) = attrs.allow_extra {
        return Err(syn::Error::new(
            span,
            "`allow_extra` is for a command without fields; to take all the text, mark the last field \
             #[command(rest)]",
        ));
    }

    let vars: Vec<Ident> = (0..fields.len()).map(|i| format_ident!("__arg{}", i)).collect();
    let construct = match &variant.fields {
        Fields::Named(_) => {
            let names = fields.iter().map(|f| &f.ident);
            quote!(Self::#ident { #(#names: #vars),* })
        }
        _ => quote!(Self::#ident(#(#vars),*)),
    };

    let mut rest = None;
    for (i, field) in fields.iter().enumerate() {
        let Some(span) = field_rest(&field.attrs)? else { continue };
        if attrs.parse_with.is_some() {
            return Err(syn::Error::new(span, "`rest` does nothing with `parse_with`, which gets all the text"));
        }
        if i + 1 != fields.len() {
            return Err(syn::Error::new(span, "only the last field can take the rest of the text"));
        }
        rest = Some(i);
    }

    if let Some(path) = &attrs.parse_with {
        let pattern = match vars.as_slice() {
            [var] => quote!(#var),
            _ => quote!((#(#vars),*)),
        };
        return Ok(quote!({
            let #pattern = #path(__command.rest().trim_end()).map_err(#krate::commands::CommandParseError::custom)?;
            ::core::result::Result::Ok(#construct)
        }));
    }

    let field_name = |i: usize, field: &Field| match &field.ident {
        Some(ident) => ident.unraw().to_string(),
        None if fields.len() == 1 => "argument".to_owned(),
        None => format!("argument {}", i + 1),
    };
    // One field takes all the text, read as a hand-written impl reads it with `parse_rest` or `parse_opt_rest`.
    if let [field] = fields.as_slice() {
        let var = &vars[0];
        let name = field_name(0, field);
        let parse = if is_option(&field.ty) { quote!(parse_opt_rest) } else { quote!(parse_rest) };
        return Ok(quote!({
            let #var = #krate::commands::#parse(__command.rest(), #name)?;
            ::core::result::Result::Ok(#construct)
        }));
    }
    let head = rest.unwrap_or(fields.len());
    let mut required = 0;
    let mut stmts = Vec::with_capacity(fields.len());
    for (i, field) in fields[..head].iter().enumerate() {
        let var = &vars[i];
        let name = field_name(i, field);
        let at = Literal::usize_unsuffixed(i);
        if is_option(&field.ty) {
            if rest.is_some() {
                return Err(syn::Error::new_spanned(
                    &field.ty,
                    "an optional field can't come before the #[command(rest)] field; make it required, or drop `rest`",
                ));
            }
            stmts.push(quote!(let #var = #krate::commands::parse_opt_arg(__args.get(#at).copied(), #at, #name)?;));
        } else {
            if required != i {
                return Err(syn::Error::new_spanned(
                    &field.ty,
                    "a required field can't follow an optional one: only the last fields can be left out",
                ));
            }
            required += 1;
            stmts.push(quote!(let #var = #krate::commands::parse_arg(__args[#at], #at, #name)?;));
        }
    }

    let split = match rest {
        None => {
            let (required, total) = (Literal::usize_unsuffixed(required), Literal::usize_unsuffixed(head));
            quote!(let __args = #krate::commands::split_args(__command.rest(), #required, #total)?;)
        }
        Some(i) => {
            let field = fields[i];
            let var = &vars[i];
            let name = field_name(i, field);
            let at = Literal::usize_unsuffixed(i);
            let tail_required = !is_option(&field.ty);
            let parse = if tail_required {
                quote!(#krate::commands::parse_arg(__tail, #at, #name)?)
            } else {
                quote!(#krate::commands::parse_opt_arg((!__tail.is_empty()).then_some(__tail), #at, #name)?)
            };
            stmts.push(quote!(let #var = #parse;));
            quote!(let (__args, __tail) = #krate::commands::split_args_rest(__command.rest(), #at, #tail_required)?;)
        }
    };
    Ok(quote!({ #split #(#stmts)* ::core::result::Result::Ok(#construct) }))
}

/// Whether the field is an `Option<T>`, which may be left out.
fn is_option(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => {
            path.qself.is_none()
                && path.path.segments.last().is_some_and(|segment| {
                    segment.ident == "Option" && matches!(segment.arguments, PathArguments::AngleBracketed(_))
                })
        }
        Type::Group(group) => is_option(&group.elem),
        Type::Paren(paren) => is_option(&paren.elem),
        _ => false,
    }
}

fn generate(input: &DeriveInput, attrs: &EnumAttrs, commands: &[Command]) -> TokenStream {
    let EnumAttrs { prefixes, case_sensitive, description, krate, .. } = attrs;
    let enum_ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let branches = commands.iter().map(|command| {
        let mut matches = TokenStream::new();
        for (i, (name, _)) in command.names.iter().enumerate() {
            if i > 0 {
                matches.extend(quote!(||));
            }
            matches.extend(if *case_sensitive {
                quote!(__command.name == #name)
            } else {
                quote!(#krate::commands::name_matches(__command.name, #name))
            });
        }
        let parse = &command.parse;
        quote!(if #matches { return #parse; })
    });

    let all_names = commands.iter().flat_map(|c| c.names.iter().map(|(name, _)| name));
    let filter = if *case_sensitive {
        quote!(#krate::filters::command_with_prefixes_case_sensitive)
    } else {
        quote!(#krate::filters::command_with_prefixes)
    };

    let menu = commands.iter().filter(|c| c.in_menu).map(|c| {
        let (name, description) = (&c.names[0].0, &c.description);
        let ephemeral = c.attrs.ephemeral.then(|| quote!(.is_ephemeral(true)));
        quote!(#krate::types::BotCommand::new(#name, #description) #ephemeral)
    });

    let prefix = &prefixes[0];
    let shown: Vec<TokenStream> = commands
        .iter()
        .filter(|c| !c.attrs.hide)
        .map(|c| {
            let (name, description) = (&c.names[0].0, &c.description);
            let aliases = (!c.attrs.hide_aliases && c.names.len() > 1).then(|| {
                let aliases = c.names[1..].iter().map(|(alias, _)| alias);
                quote!(.aliases(&[#(#aliases),*]))
            });
            quote!(#krate::commands::CommandDescription::new(#prefix, #name, #description) #aliases)
        })
        .collect();
    let shown_count = Literal::usize_unsuffixed(shown.len());
    let header = description.as_ref().map(|header| quote!(.header(#header)));

    quote! {
        #[automatically_derived]
        impl #impl_generics #krate::commands::BotCommands for #enum_ident #ty_generics #where_clause {
            fn parse(
                __text: &str,
                __bot_username: ::core::option::Option<&str>,
            ) -> ::core::result::Result<Self, #krate::commands::CommandParseError> {
                let __command = #krate::commands::parse_header(__text, &[#(#prefixes),*], __bot_username)?;
                #(#branches)*
                ::core::result::Result::Err(#krate::commands::CommandParseError::UnknownCommand(
                    ::std::string::String::from(__command.name),
                ))
            }

            fn filter<__VaegramText: #krate::filters::HasText>() -> #krate::filters::Filter<__VaegramText> {
                #filter([#(#prefixes),*], [#(#all_names),*])
            }

            fn bot_commands() -> ::std::vec::Vec<#krate::types::BotCommand> {
                ::std::vec![#(#menu),*]
            }

            fn descriptions() -> #krate::commands::CommandDescriptions<'static> {
                static COMMANDS: [#krate::commands::CommandDescription<'static>; #shown_count] = [#(#shown),*];
                #krate::commands::CommandDescriptions::new(&COMMANDS) #header
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::bot_commands;

    /// The build errors the derive reports for the item `source`, in order.
    fn errors(source: &str) -> Vec<String> {
        match bot_commands(&syn::parse_str(source).unwrap()) {
            Ok(_) => Vec::new(),
            Err(error) => error.into_iter().map(|e| e.to_string()).collect(),
        }
    }

    /// Fails unless the derive refuses `source` with exactly one error containing `needle`.
    fn refuses(source: &str, needle: &str) {
        let errors = errors(source);
        assert!(errors.len() == 1 && errors[0].contains(needle), "{needle:?} not in {errors:?}");
    }

    /// Fails unless the derive takes `source`.
    fn takes(source: &str) {
        let errors = errors(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }

    #[test]
    fn only_enums_with_variants() {
        refuses("struct Cmd;", "derives only for an enum");
        refuses("enum Cmd {}", "at least one variant");
    }

    #[test]
    fn menu_commands_need_a_valid_name_and_description() {
        refuses("enum Cmd { Start }", "needs a description of 3 to 256 characters, and this one has 0");
        refuses(r#"enum Cmd { #[doc = "Go"] Start }"#, "this one has 2");
        refuses(&format!(r#"enum Cmd {{ #[doc = "{}"] Start }}"#, "a".repeat(257)), "this one has 257");
        refuses(
            r#"#[command(rename_rule = "PascalCase")] enum Cmd { #[doc = "Start the bot"] Start }"#,
            "only lowercase English letters",
        );
        // Out of the menu, neither rule applies.
        takes("#[command(hide_menu)] enum Cmd { Start, SetAge }");
        takes(r#"enum Cmd { #[command(hide)] Start, #[command(hide_menu, rename = "Go")] Go }"#);
    }

    #[test]
    fn every_error_is_reported_in_one_build() {
        let errors = errors(r#"enum Cmd { Start, Help, #[command(hide, rename = "a-b")] Debug }"#);
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert!(errors[2].contains("`a-b` can't be a command"), "{errors:?}");
    }

    #[test]
    fn names_and_aliases_name_one_command() {
        refuses(
            r#"#[command(hide_menu)] enum Cmd { Start, #[command(alias = "START")] Begin }"#,
            "`START` already names the command of `Start`",
        );
        // With exact case, `START` and `start` are two commands.
        takes(r#"#[command(hide_menu, case_sensitive)] enum Cmd { Start, #[command(alias = "START")] Begin }"#);
        refuses(r#"#[command(hide_menu)] enum Cmd { #[command(alias = "GO")] Go }"#, "`GO` already");
    }

    #[test]
    fn the_menu_holds_100_commands() {
        let variants: String = (0..101).map(|i| format!("#[doc = \"Command {i}\"] C{i},")).collect();
        refuses(&format!("enum Cmd {{ {variants} }}"), "at most 100 commands and this enum shows 101");
    }

    #[test]
    fn misplaced_settings_are_refused() {
        refuses(r#"#[command(rename_rule = "kebab-case")] enum Cmd { Go }"#, "can't contain `-`");
        refuses(r#"#[command(prefix = "")] enum Cmd { Go }"#, "prefix can't be empty");
        refuses("#[command(prefixes = [])] enum Cmd { Go }", "at least one prefix");
        refuses(r#"#[command(hide_menu, colour = "red")] enum Cmd { Go }"#, "unknown enum setting");
        refuses("#[command(hide_menu)] enum Cmd { #[command(hidden)] Go }", "unknown command setting");
        refuses("#[command(hide_menu)] enum Cmd { #[command(rest)] Go(String) }", "goes on the last field");
        refuses("#[command(hide_menu)] enum Cmd { #[command(allow_extra)] Go(String) }", "`allow_extra`");
        refuses("#[command(hide_menu)] enum Cmd { #[command(parse_with = p)] Go }", "no fields");
        refuses(r#"#[command(hide_menu)] enum Cmd { #[command(rename = "a", rename = "b")] Go }"#, "twice");
    }

    #[test]
    fn fields_must_split_one_way() {
        refuses("#[command(hide_menu)] enum Cmd { Go(Option<u8>, u8) }", "can't follow an optional one");
        refuses("#[command(hide_menu)] enum Cmd { Go(#[command(rest)] String, u8) }", "only the last field");
        refuses(
            "#[command(hide_menu)] enum Cmd { Go(Option<u8>, #[command(rest)] String) }",
            "before the #[command(rest)] field",
        );
        refuses(
            "#[command(hide_menu)] enum Cmd { #[command(parse_with = p)] Go(#[command(rest)] String) }",
            "`rest` does nothing with `parse_with`",
        );
        refuses("#[command(hide_menu)] enum Cmd { Go(#[command(tail)] String) }", "unknown field setting");
        takes("#[command(hide_menu)] enum Cmd { Go(u8, Option<u8>, Option<String>) }");
        takes("#[command(hide_menu)] enum Cmd { Go { a: u8, #[command(rest)] b: String } }");
    }

    #[test]
    fn doc_comments_give_the_first_paragraph() {
        let source = "enum Cmd {\n/// Start\n/// the bot\n///\n/// Not for users.\nStart }";
        let code = bot_commands(&syn::parse_str(source).unwrap()).unwrap().to_string();
        assert!(code.contains("\"Start the bot\"") && !code.contains("Not for users"), "{code}");
    }
}
