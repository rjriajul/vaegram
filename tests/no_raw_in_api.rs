//! The friendly API takes and returns friendly types only: a TL type (`raw::base::*`, `raw::types::*`) in a public
//! signature would tie every caller to the TL schema of one layer. This scans `src/` outside `src/raw/` and reports
//! a TL type named in:
//! - a `pub fn` signature, or a method, associated type or const in a `pub trait`;
//! - a `pub` field, a `pub struct` tuple field, a variant of a `pub enum`, a `pub type`, `pub const` or `pub static`;
//! - an `impl` header (`impl<T> TryFrom<base::X> for Y`, `impl Trait for base::X`, …) or a `type X =` inside an
//!   `impl`, including the `IntoFuture` impls `into_future!` writes;
//! - a `pub use`.
//!
//! A TL type is a path through `raw::base`/`raw::types`, through `base`/`types` imported from `crate::raw` or
//! `base` from `super`, or a name imported from them (aliases and nested `use` groups included).
//!
//! Skipped: `#[cfg(test)]` modules and files, items gated by `#[cfg(feature = "unstable-raw")]` or `#[cfg(test)]`,
//! and anything inside a type, trait, `impl` or inline module that isn't plain `pub` (`pub(crate)`, private). The
//! scan reads lines, not types: it doesn't follow type aliases, macros other than `into_future!`, or the visibility
//! of modules declared in other files.

use std::fs;
use std::path::Path;

/// `(file, item)` pairs that may name TL types, and why.
const ALLOWED: &[(&str, &str)] = &[
    // The chat id converters between Bot API ids and TL peers, for code built on `invoke`.
    ("src/utils.rs", "get_peer_id"),
    ("src/utils.rs", "get_peer"),
    // The input half of the raw escape hatch next to `invoke`: the TL peer, with its access hash, for a chat.
    ("src/methods/advanced/resolve_peer.rs", "resolve_peer"),
];

#[test]
fn friendly_signatures_name_no_tl_types() {
    let mut files = Vec::new();
    collect(Path::new("src"), &mut files);
    let test_modules = test_module_files(&files);
    let mut found = Vec::new();
    let mut allowed_hits = Vec::new();
    for (path, source) in &files {
        if path.starts_with("src/raw/") || test_modules.contains(path) {
            continue;
        }
        for (line, item, text) in violations(source) {
            if ALLOWED.contains(&(path.as_str(), item.as_str())) {
                allowed_hits.push((path.clone(), item));
            } else {
                found.push(format!("{path}:{line} `{item}`: {text}"));
            }
        }
    }
    assert!(found.is_empty(), "TL types in friendly public signatures:\n{}", found.join("\n"));
    for &(path, item) in ALLOWED {
        let hit = allowed_hits.iter().any(|(p, i)| p == path && i == item);
        assert!(hit, "stale ALLOWED entry: {path} `{item}` no longer names a TL type");
    }
}

#[test]
fn the_scan_finds_what_it_should() {
    let cases: &[(&str, &[&str])] = &[
        // Fields, variants, aliases and functions; hidden owners, gates and test modules.
        (
            r#"use crate::raw::{base, types};
use crate::raw::base::InputPeer;
use crate::types::User;

pub struct Friendly {
    pub user: User,
    pub raw: base::User,
    pub(crate) hidden: base::Chat,
}

pub struct Wrapper(pub base::Message);

pub(crate) struct Hidden {
    pub raw: base::User,
}

pub enum Kind {
    Known(User),
    Raw(types::Message),
}

pub type Alias = base::Peer;
pub const EMPTY: Option<base::Peer> = None;

impl Client {
    pub async fn get_me(&self) -> Result<User> {}
    pub async fn leaks(
        &self,
        peer: InputPeer,
    ) -> Result<()> {}
    pub(crate) async fn internal(&self) -> base::User {}
    #[cfg(feature = "unstable-raw")]
    pub fn raw(&self) -> &base::Update {}
    pub fn friendly(&self) -> crate::types::Message {}
}

impl Hidden {
    pub fn raw(&self) -> base::User {}
}

#[cfg(feature = "unstable-raw")]
impl Friendly {
    pub fn into_raw(self) -> base::InputFile {}
}

#[cfg(test)]
mod tests {
    pub fn helper() -> base::User {}
}
"#,
            &["raw", "Wrapper", "Raw", "Alias", "EMPTY", "leaks"],
        ),
        // `impl` headers of any shape, and associated types inside impls.
        (
            r#"use crate::raw::base;

pub struct A;
struct Private;

impl<'a> From<base::User> for A {}
impl TryFrom<base::User> for A {}
impl Into<base::User> for A {}
impl PartialEq<base::Message> for A {}
impl AsRef<base::User> for A {}
impl Searchable for base::Message {}
impl From<base::User> for Private {}

impl std::ops::Deref for A {
    type Target = base::User;
}

impl Iterator for A {
    type Item = base::Message;
}

impl IntoFuture for Fetch<'_> {
    type Output = Result<base::Updates>;
}

into_future!(Upload, base::InputFile);
into_future!(Private, base::InputFile);
"#,
            &[
                "impl A",
                "impl A",
                "impl A",
                "impl A",
                "impl A",
                "impl base::Message",
                "A::Target",
                "A::Item",
                "Fetch::Output",
                "impl Upload",
            ],
        ),
        // Re-exports, with and without `as`.
        (
            r#"pub use crate::raw::base::User;
pub use crate::raw::base::Chat as RawChat;
pub use crate::types::Message;
pub(crate) use crate::raw::base::Peer;
"#,
            &["User", "RawChat"],
        ),
        // Nested `use` groups, `self` and aliases.
        (
            r#"use crate::raw::{base::{self as tl, User as TlUser}, functions};

pub fn module_alias() -> tl::Message {}
pub fn name_alias() -> TlUser {}
pub fn friendly() -> crate::types::User {}
"#,
            &["module_alias", "name_alias"],
        ),
        // Inline modules: `use super::base`, `super::base::` paths, and owners inside them.
        (
            r#"pub mod open {
    use super::base;

    pub struct B;

    impl From<super::base::User> for B {}

    pub fn through_super() -> base::User {}
}

mod closed {
    pub struct C;

    impl From<super::base::User> for C {}

    pub fn hidden() -> super::base::User {}
}
"#,
            &["impl B", "through_super"],
        ),
        // Variants split over lines.
        (
            r#"use crate::raw::base;

pub enum Event {
    Plain(u32),
    Struct {
        id: i64,
        raw: base::User,
    },
    Split(
        i64,
        base::Message,
    ),
    Friendly {
        id: i64,
    },
}
"#,
            &["Struct", "Split"],
        ),
        // Trait bodies.
        (
            r#"use crate::raw::base;

pub trait Public {
    type Raw: Into<base::User>;
    fn raw(&self) -> base::User;
    fn friendly(&self) -> crate::types::User;
}

pub(crate) trait Internal {
    fn raw(&self) -> base::User;
}
"#,
            &["Public::Raw", "Public::raw"],
        ),
    ];
    for (source, expected) in cases {
        let items: Vec<String> = violations(source).into_iter().map(|(_, item, _)| item).collect();
        assert_eq!(&items, expected, "in:\n{source}");
    }
}

fn collect(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let name = path.to_string_lossy().replace('\\', "/");
            out.push((name, fs::read_to_string(&path).unwrap().replace("\r\n", "\n")));
        }
    }
}

/// Files that are `#[cfg(test)] mod name;` of another file.
fn test_module_files(files: &[(String, String)]) -> Vec<String> {
    let mut modules = Vec::new();
    for (path, source) in files {
        let dir = match path.strip_suffix("/mod.rs") {
            Some(dir) => dir.to_string(),
            None => path.trim_end_matches(".rs").to_string(),
        };
        let lines: Vec<&str> = source.lines().map(str::trim).collect();
        for pair in lines.windows(2) {
            if pair[0] != "#[cfg(test)]" {
                continue;
            }
            let decl = pair[1].strip_prefix("pub(crate) ").unwrap_or(pair[1]);
            if let Some(name) = decl.strip_prefix("mod ").and_then(|d| d.strip_suffix(';')) {
                modules.push(format!("{dir}/{name}.rs"));
            }
        }
    }
    modules
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Type,
    Enum,
    Trait,
    Impl,
    Module,
}

/// A block the current line is inside: what opened it, at which indent, and whether it hides what it holds.
struct Scope {
    indent: usize,
    kind: Kind,
    name: String,
    hidden: bool,
}

/// `(line, item, text)` for each public signature in `source` that names a TL type.
fn violations(source: &str) -> Vec<(usize, String, String)> {
    let lines: Vec<&str> = source.lines().collect();
    let tl = TlNames::of(source);
    let private = private_types(&lines);
    let is_private = |name: &str| private.iter().any(|p| p == name);
    let mut found = Vec::new();
    let mut attrs = String::new();
    let mut scopes: Vec<Scope> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            i += 1;
            continue;
        }
        if trimmed.starts_with("#[") {
            attrs.push_str(trimmed);
            i += 1;
            continue;
        }
        let gated = attrs.contains("cfg(feature = \"unstable-raw\")") || attrs.contains("cfg(test)");
        attrs.clear();
        while scopes.last().is_some_and(|s| s.indent >= indent) {
            scopes.pop();
        }
        // A test module's strings can hold anything: skip it to its closing brace.
        if gated && trimmed.starts_with("mod ") && trimmed.ends_with('{') {
            i = end_of_block(&lines, i, indent) + 1;
            continue;
        }
        let hidden = gated || scopes.iter().any(|s| s.hidden);
        let parent = scopes.last().filter(|s| indent == s.indent + 4).map(|s| (s.kind, s.name.clone()));
        let line_number = i + 1;
        let mut report = |item: String, text: &str| {
            if !hidden && tl.found_in(text) {
                found.push((line_number, item, text.to_string()));
            }
        };

        if let Some((kind, name, public)) = declaration(trimmed) {
            let (end, header) = signature(&lines, i);
            let name = if kind == Kind::Impl { impl_target(&header) } else { name };
            let hides = if kind == Kind::Impl { is_private(&name) } else { !public };
            if kind == Kind::Impl && !hides {
                report(format!("impl {name}"), &header);
            } else if kind == Kind::Type && public && header.contains("(pub ") {
                report(name.clone(), &header);
            }
            if lines[end].contains('{') {
                scopes.push(Scope { indent, kind, name, hidden: hidden || hides });
            }
            i = end + 1;
            continue;
        }
        if let Some(name) = fn_name(trimmed) {
            let (end, header) = signature(&lines, i);
            report(name.to_string(), &header);
            i = end + 1;
            continue;
        }
        if trimmed.starts_with("pub use ") {
            let end = (i..lines.len()).find(|&j| lines[j].trim_end().ends_with(';')).unwrap_or(i);
            let statement = lines[i..=end].iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ");
            for (name, full) in flatten_use(statement.trim_start_matches("pub use ").trim_end_matches(';')) {
                report(name, &full);
            }
            i = end + 1;
            continue;
        }
        if let Some(name) = pub_item(trimmed) {
            report(name.to_string(), trimmed);
        } else if let Some(args) = trimmed.strip_prefix("into_future!(") {
            let name = args.split(',').next().unwrap_or(args).trim();
            if !is_private(name) {
                report(format!("impl {name}"), trimmed);
            }
        } else if let Some((kind, owner)) = parent {
            let member = ["type ", "fn ", "const "].iter().find_map(|word| trimmed.strip_prefix(word));
            let member = member.map(|rest| rest.split([' ', ':', '=', '<', '(', ';']).next().unwrap_or(rest));
            match (kind, member) {
                (Kind::Impl, Some(name)) if trimmed.starts_with("type ") => {
                    report(format!("{owner}::{name}"), trimmed);
                }
                (Kind::Trait, Some(name)) => {
                    let (end, header) = signature(&lines, i);
                    report(format!("{owner}::{name}"), &header);
                    i = end;
                }
                (Kind::Enum, None) => {
                    let name = trimmed.split(['(', ' ', '{', ',']).next().unwrap_or(trimmed).to_string();
                    let (end, text) = match trimmed.ends_with(['{', '(']) {
                        true => balanced(&lines, i),
                        false => (i, trimmed.to_string()),
                    };
                    report(name, &text);
                    i = end;
                }
                _ => {}
            }
        }
        i += 1;
    }
    found
}

/// The lines from `start` to the one that closes the brackets `start` opens (a struct or tuple variant split over
/// lines), and their text.
fn balanced(lines: &[&str], start: usize) -> (usize, String) {
    let mut depth = 0i32;
    let mut end = start;
    for (j, line) in lines.iter().enumerate().skip(start) {
        depth += line.matches(['{', '(']).count() as i32 - line.matches(['}', ')']).count() as i32;
        end = j;
        if depth <= 0 {
            break;
        }
    }
    (end, lines[start..=end].iter().map(|l| l.trim()).collect::<Vec<_>>().join(" "))
}

/// The lines from `start` to the one that opens a body or ends the item, and their text up to the body.
fn signature(lines: &[&str], start: usize) -> (usize, String) {
    let end = (start..lines.len())
        .find(|&j| lines[j].contains('{') || lines[j].trim_end().ends_with(';'))
        .unwrap_or(lines.len() - 1);
    let text = lines[start..=end].iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ");
    let text = text.split('{').next().unwrap_or(&text).to_string();
    (end, text)
}

/// What a line declares, the name it declares and whether it's plain `pub`: `pub struct A`, `enum B`,
/// `pub trait C`, `impl …`, `pub mod d {`.
fn declaration(line: &str) -> Option<(Kind, String, bool)> {
    let (public, rest) = match line.strip_prefix("pub ") {
        Some(rest) => (true, rest),
        None => (false, ["pub(crate) ", "pub(super) "].iter().find_map(|p| line.strip_prefix(p)).unwrap_or(line)),
    };
    let rest = rest.strip_prefix("unsafe ").unwrap_or(rest);
    if rest.starts_with("impl ") || rest.starts_with("impl<") {
        return Some((Kind::Impl, String::new(), true));
    }
    let (kind, rest) =
        [("struct ", Kind::Type), ("enum ", Kind::Enum), ("trait ", Kind::Trait), ("mod ", Kind::Module)]
            .iter()
            .find_map(|&(word, kind)| rest.strip_prefix(word).map(|r| (kind, r)))?;
    if kind == Kind::Module && !line.ends_with('{') {
        return None;
    }
    Some((kind, ident(rest), public))
}

/// The type an `impl` header is for: `Foo` in `impl<'a> Foo<'a>`, `Bar` in `impl From<X> for Bar`, and the whole
/// path in `impl Trait for base::X`.
fn impl_target(header: &str) -> String {
    let head = header.split(" where ").next().unwrap_or(header).trim();
    let target = match head.rsplit_once(" for ") {
        Some((_, target)) => target,
        None => {
            let rest = head.trim_start_matches("unsafe ").trim_start_matches("impl");
            match rest.strip_prefix('<') {
                Some(_) => skip_generics(rest),
                None => rest,
            }
        }
    };
    target.trim().chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':').collect()
}

/// What follows the `<…>` that `s` starts with.
fn skip_generics(s: &str) -> &str {
    let mut depth = 0;
    for (at, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return &s[at + 1..];
                }
            }
            _ => {}
        }
    }
    ""
}

/// Types declared without `pub`, or with a restricted `pub(…)`.
fn private_types(lines: &[&str]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|l| match declaration(l.trim())? {
            (Kind::Type | Kind::Enum | Kind::Trait, name, false) => Some(name),
            _ => None,
        })
        .collect()
}

fn ident(s: &str) -> String {
    s.trim().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect()
}

/// The name of a `pub fn` (not `pub(crate)`), if the line starts one.
fn fn_name(line: &str) -> Option<&str> {
    let mut rest = line.strip_prefix("pub ")?;
    for qualifier in ["const ", "async ", "unsafe ", "extern \"C\" "] {
        rest = rest.strip_prefix(qualifier).unwrap_or(rest);
    }
    let rest = rest.strip_prefix("fn ")?;
    rest.split(['(', '<']).next()
}

/// The name of a `pub` field, type alias, const or static, if the line declares one.
fn pub_item(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("pub ")?;
    if let Some(rest) = rest.strip_prefix("type ") {
        return rest.split([' ', '<', '=']).next();
    }
    let rest = rest.strip_prefix("const ").or_else(|| rest.strip_prefix("static ")).unwrap_or(rest);
    let (name, _) = rest.split_once(':')?;
    name.chars().all(|c| c.is_alphanumeric() || c == '_').then_some(name)
}

/// Each name a `use` tree brings in, with the full path it stands for: `a::{b::{self as c, D}, e}` gives
/// `(c, a::b)`, `(D, a::b::D)` and `(e, a::e)`.
fn flatten_use(tree: &str) -> Vec<(String, String)> {
    fn walk(prefix: &str, tree: &str, out: &mut Vec<(String, String)>) {
        let tree = tree.trim();
        let Some(open) = tree.find('{') else {
            let (path, alias) = match tree.split_once(" as ") {
                Some((path, alias)) => (path.trim(), Some(alias.trim())),
                None => (tree, None),
            };
            let full = match (prefix.is_empty(), path) {
                (_, "self") => prefix.trim_end_matches("::").to_string(),
                (true, path) => path.to_string(),
                (false, path) => format!("{prefix}{path}"),
            };
            let name = alias.unwrap_or_else(|| full.rsplit("::").next().unwrap_or(&full)).to_string();
            if !name.is_empty() && name != "*" {
                out.push((name, full));
            }
            return;
        };
        let prefix = format!("{prefix}{}", &tree[..open]);
        let close = tree.rfind('}').filter(|&close| close > open).unwrap_or(tree.len());
        let inner = &tree[open + 1..close];
        let mut depth = 0;
        let mut start = 0;
        for (at, c) in inner.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                ',' if depth == 0 => {
                    walk(&prefix, &inner[start..at], out);
                    start = at + 1;
                }
                _ => {}
            }
        }
        walk(&prefix, &inner[start..], out);
    }
    let mut out = Vec::new();
    walk("", tree, &mut out);
    out
}

/// The ways a file can spell a TL type: through `raw::base`/`raw::types`, through the `base`/`types` modules it
/// imports from `crate::raw` (or `base` from `super`), and the names it imports from them.
struct TlNames {
    modules: Vec<String>,
    names: Vec<String>,
}

impl TlNames {
    fn of(source: &str) -> TlNames {
        let mut modules = Vec::new();
        let mut names = Vec::new();
        for statement in source.split(';') {
            let Some(line) = statement.lines().position(|l| {
                let l = l.trim_start();
                ["use ", "pub use ", "pub(crate) use ", "pub(super) use "].iter().any(|p| l.starts_with(p))
            }) else {
                continue;
            };
            let statement = statement.lines().skip(line).collect::<Vec<_>>().join(" ");
            let statement = statement.split_whitespace().collect::<Vec<_>>().join(" ");
            let tree = &statement[statement.find("use ").unwrap_or(0) + "use ".len()..];
            for (name, full) in flatten_use(tree) {
                let module = full.rsplit("::").next().unwrap_or("");
                if full.ends_with("raw::base")
                    || full.ends_with("raw::types")
                    || (full.starts_with("super::") && module == "base")
                {
                    modules.push(name);
                } else if TlNames::is_tl_path(&full) {
                    names.push(name);
                }
            }
        }
        TlNames { modules, names }
    }

    /// A path through `raw::base`/`raw::types` to something inside them.
    fn is_tl_path(path: &str) -> bool {
        path.contains("raw::base::") || path.contains("raw::types::")
    }

    fn found_in(&self, text: &str) -> bool {
        if TlNames::is_tl_path(text) {
            return true;
        }
        // Not part of a longer path, except one that only climbs modules (`super::base::`).
        let bare = |at: usize| {
            let mut before = &text[..at];
            while let Some(rest) = before.strip_suffix("super::").or_else(|| before.strip_suffix("self::")) {
                before = rest;
            }
            !before.ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == ':')
        };
        let climbs = text.contains("super::base::").then_some("base");
        let in_module = self
            .modules
            .iter()
            .map(String::as_str)
            .chain(climbs)
            .any(|module| text.match_indices(&format!("{module}::")).any(|(at, _)| bare(at)));
        in_module
            || self.names.iter().any(|name| {
                text.match_indices(name.as_str()).any(|(at, _)| {
                    let after = text[at + name.len()..].chars().next();
                    bare(at) && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
                })
            })
    }
}

/// The line of the `}` that closes the block opened on `start`, at `indent`.
fn end_of_block(lines: &[&str], start: usize, indent: usize) -> usize {
    (start + 1..lines.len())
        .find(|&i| lines[i].trim() == "}" && lines[i].len() - lines[i].trim_start().len() == indent)
        .unwrap_or(lines.len() - 1)
}
