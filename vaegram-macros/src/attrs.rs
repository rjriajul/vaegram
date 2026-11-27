//! The `#[command(...)]` attributes on the enum, its variants and their fields, and the variants' doc comments.

use proc_macro2::Span;
use syn::meta::ParseNestedMeta;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, ExprLit, Lit, LitStr, Meta, Path, Token};

use crate::rename::RenameRule;

/// Settings for the whole enum.
pub(crate) struct EnumAttrs {
    pub(crate) rename_rule: RenameRule,
    /// The prefixes, never empty; the first one shows in the help text.
    pub(crate) prefixes: Vec<String>,
    pub(crate) case_sensitive: bool,
    /// The help text's header.
    pub(crate) description: Option<String>,
    /// Keeps every command out of the menu.
    pub(crate) hide_menu: bool,
    /// The path of the vaegram crate in generated code.
    pub(crate) krate: Path,
}

impl EnumAttrs {
    pub(crate) fn parse(attrs: &[Attribute]) -> syn::Result<EnumAttrs> {
        let mut rename_rule = None;
        let mut prefixes: Option<Vec<LitStr>> = None;
        let mut case_sensitive = false;
        let mut description = None;
        let mut hide_menu = false;
        let mut krate = None;
        for attr in attrs.iter().filter(|a| a.path().is_ident("command")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename_rule") {
                    let lit: LitStr = meta.value()?.parse()?;
                    once(&mut rename_rule, RenameRule::parse(&lit)?, &meta)
                } else if meta.path.is_ident("prefix") {
                    let lit: LitStr = meta.value()?.parse()?;
                    once(&mut prefixes, vec![lit], &meta)
                } else if meta.path.is_ident("prefixes") {
                    let list = lit_list(&meta)?;
                    if list.is_empty() {
                        return Err(meta.error("give at least one prefix, such as `prefixes = [\"/\"]`"));
                    }
                    once(&mut prefixes, list, &meta)
                } else if meta.path.is_ident("case_sensitive") {
                    case_sensitive = true;
                    Ok(())
                } else if meta.path.is_ident("description") {
                    let lit: LitStr = meta.value()?.parse()?;
                    once(&mut description, lit.value(), &meta)
                } else if meta.path.is_ident("hide_menu") {
                    hide_menu = true;
                    Ok(())
                } else if meta.path.is_ident("crate") {
                    let lit: LitStr = meta.value()?.parse()?;
                    once(&mut krate, lit.parse::<Path>()?, &meta)
                } else {
                    Err(meta.error(
                        "unknown enum setting; expected `rename_rule`, `prefix`, `prefixes`, `case_sensitive`, \
                         `description`, `hide_menu` or `crate`",
                    ))
                }
            })?;
        }
        let prefixes = prefixes.unwrap_or_default();
        if let Some(empty) = prefixes.iter().find(|p| p.value().is_empty()) {
            return Err(syn::Error::new(empty.span(), "a command prefix can't be empty"));
        }
        let prefixes =
            if prefixes.is_empty() { vec!["/".to_owned()] } else { prefixes.iter().map(LitStr::value).collect() };
        Ok(EnumAttrs {
            rename_rule: rename_rule.unwrap_or(RenameRule::SnakeCase),
            prefixes,
            case_sensitive,
            description,
            hide_menu,
            krate: krate.unwrap_or_else(|| syn::parse_quote!(::vaegram)),
        })
    }
}

/// Settings for one variant, its command.
pub(crate) struct VariantAttrs {
    pub(crate) rename: Option<LitStr>,
    pub(crate) aliases: Vec<LitStr>,
    /// From `description = "..."`, or else the first paragraph of the doc comment.
    pub(crate) description: Option<String>,
    pub(crate) hide: bool,
    pub(crate) hide_menu: bool,
    pub(crate) hide_aliases: bool,
    pub(crate) ephemeral: bool,
    /// Where `allow_extra` was written, if it was.
    pub(crate) allow_extra: Option<Span>,
    pub(crate) parse_with: Option<Path>,
}

impl VariantAttrs {
    pub(crate) fn parse(attrs: &[Attribute]) -> syn::Result<VariantAttrs> {
        let mut out = VariantAttrs {
            rename: None,
            aliases: Vec::new(),
            description: None,
            hide: false,
            hide_menu: false,
            hide_aliases: false,
            ephemeral: false,
            allow_extra: None,
            parse_with: None,
        };
        for attr in attrs.iter().filter(|a| a.path().is_ident("command")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    let lit: LitStr = meta.value()?.parse()?;
                    once(&mut out.rename, lit, &meta)
                } else if meta.path.is_ident("alias") {
                    out.aliases.push(meta.value()?.parse()?);
                    Ok(())
                } else if meta.path.is_ident("aliases") {
                    out.aliases.extend(lit_list(&meta)?);
                    Ok(())
                } else if meta.path.is_ident("description") {
                    let lit: LitStr = meta.value()?.parse()?;
                    once(&mut out.description, lit.value(), &meta)
                } else if meta.path.is_ident("hide") {
                    out.hide = true;
                    Ok(())
                } else if meta.path.is_ident("hide_menu") {
                    out.hide_menu = true;
                    Ok(())
                } else if meta.path.is_ident("hide_aliases") {
                    out.hide_aliases = true;
                    Ok(())
                } else if meta.path.is_ident("ephemeral") {
                    out.ephemeral = true;
                    Ok(())
                } else if meta.path.is_ident("allow_extra") {
                    out.allow_extra = Some(meta.path.span());
                    Ok(())
                } else if meta.path.is_ident("parse_with") {
                    let path: Path = meta.value()?.parse()?;
                    once(&mut out.parse_with, path, &meta)
                } else if meta.path.is_ident("rest") {
                    Err(meta.error("`rest` goes on the last field: `Ban(i64, #[command(rest)] String)`"))
                } else {
                    Err(meta.error(
                        "unknown command setting; expected `rename`, `alias`, `aliases`, `description`, `hide`, \
                         `hide_menu`, `hide_aliases`, `ephemeral`, `allow_extra` or `parse_with`",
                    ))
                }
            })?;
        }
        if out.description.is_none() {
            out.description = doc_paragraph(attrs);
        }
        Ok(out)
    }
}

/// Where a field's `#[command(rest)]` was written, if it was.
pub(crate) fn field_rest(attrs: &[Attribute]) -> syn::Result<Option<Span>> {
    let mut rest = None;
    for attr in attrs.iter().filter(|a| a.path().is_ident("command")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rest") {
                rest = Some(meta.path.span());
                Ok(())
            } else {
                Err(meta.error("unknown field setting; the only one is `rest`"))
            }
        })?;
    }
    Ok(rest)
}

/// The first paragraph of the doc comment, its lines trimmed and joined with spaces. Later paragraphs stay
/// rustdoc only, so notes for developers don't reach the bot's users.
fn doc_paragraph(attrs: &[Attribute]) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("doc")) {
        let Meta::NameValue(doc) = &attr.meta else { continue };
        let Expr::Lit(ExprLit { lit: Lit::Str(text), .. }) = &doc.value else { continue };
        // `split`, not `lines`: a bare `///` is an empty string, which `lines` yields nothing for, and it is the
        // paragraph break.
        for line in text.value().split('\n').map(str::trim) {
            if !line.is_empty() {
                lines.push(line.to_owned());
            } else if !lines.is_empty() {
                return Some(lines.join(" "));
            }
        }
    }
    (!lines.is_empty()).then(|| lines.join(" "))
}

/// `["a", "b"]` after `=`.
fn lit_list(meta: &ParseNestedMeta<'_>) -> syn::Result<Vec<LitStr>> {
    let input = meta.value()?;
    let content;
    syn::bracketed!(content in input);
    Ok(Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?.into_iter().collect())
}

/// Sets a setting that may appear once.
fn once<T>(slot: &mut Option<T>, value: T, meta: &ParseNestedMeta<'_>) -> syn::Result<()> {
    if slot.is_some() {
        return Err(meta.error("this setting is given twice"));
    }
    *slot = Some(value);
    Ok(())
}
