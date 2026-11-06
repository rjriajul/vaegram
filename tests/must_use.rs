use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// Macros that implement `IntoFuture` for the type named as their first argument.
const BUILDER_MACROS: [&str; 4] = ["into_future!(", "send_builder!(", "send_media!(", "pager!("];

/// Types with a `next` that are not pagers: a chunk stream, whose file `DownloadMedia` already collects when awaited.
const STREAMS: [&str; 1] = ["MediaStream"];

/// Types whose `self -> Self` methods take effect at once and hand `self` back only for chaining: a handle to a
/// registered handler, which stays registered when the handle is dropped.
const SIDE_EFFECT_SETTERS: [&str; 1] = ["HandlerRef"];

fn sources(dir: &Path, files: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, files);
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(fs::read_to_string(&path).unwrap());
        }
    }
}

fn ident(text: &str) -> &str {
    let text = text.trim_start();
    let end = text.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(text.len());
    &text[..end]
}

/// Every type something implements `IntoFuture` for: a request that does nothing until awaited.
fn builders(source: &str, names: &mut BTreeSet<String>) {
    for invocation in BUILDER_MACROS {
        for (at, _) in source.match_indices(invocation) {
            names.insert(ident(&source[at + invocation.len()..]).to_owned());
        }
    }
    for (at, _) in source.match_indices("IntoFuture for ") {
        names.insert(ident(&source[at + "IntoFuture for ".len()..]).to_owned());
    }
    // Macro bodies name their argument, not a type.
    names.retain(|n| !n.starts_with('$') && !n.is_empty());
}

/// Each struct or enum declared in `source`, and whether an attribute or doc line above it holds `#[must_use`.
fn declarations(source: &str, types: &mut BTreeMap<String, bool>) {
    let lines: Vec<&str> = source.lines().map(str::trim).collect();
    for (i, line) in lines.iter().enumerate() {
        let item = line.strip_prefix("pub(crate) ").or_else(|| line.strip_prefix("pub ")).unwrap_or(line);
        let Some(rest) = item.strip_prefix("struct ").or_else(|| item.strip_prefix("enum ")) else { continue };
        let above = lines[..i].iter().rev().take_while(|l| l.starts_with("#[") || l.starts_with("//"));
        let must_use = above.into_iter().any(|l| l.starts_with("#[must_use"));
        *types.entry(ident(rest).to_owned()).or_default() |= must_use;
    }
}

/// Every type with an inherent `pub async fn next(&mut self)`: the pagers, plus `STREAMS`.
fn pagers(source: &str, names: &mut BTreeSet<String>) {
    let mut current = None;
    for line in source.lines() {
        if let Some(rest) = line.strip_prefix("impl") {
            // `impl Name<'_> {` or `impl<'a> Name<'a> {`; trait impls have no inherent `next`.
            let rest = match rest.strip_prefix('<') {
                Some(generics) => generics.split_once('>').map_or("", |(_, rest)| rest),
                None => rest,
            };
            current = (!rest.contains(" for ")).then(|| ident(rest));
        } else if line.trim_start().starts_with("pub async fn next(&mut self)") {
            names.extend(current.map(str::to_owned));
        }
    }
}

/// Skips a leading `<…>` generics list, nesting included.
fn skip_generics(text: &str) -> &str {
    let text = text.trim_start();
    if !text.starts_with('<') {
        return text;
    }
    let mut depth = 0;
    for (at, c) in text.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return &text[at + 1..];
        }
    }
    ""
}

/// Whether a signature from `fn` on takes `self` by value and returns `Self`: a chainable setter.
fn is_chainable(signature: &str) -> bool {
    let Some(params) = skip_generics(signature.trim_start_matches(|c: char| c.is_alphanumeric() || c == '_'))
        .trim_start()
        .strip_prefix('(')
    else {
        return false;
    };
    // Only parentheses: `->` in an `impl Fn() -> T` parameter would unbalance angle brackets.
    let mut depth = 1;
    let Some(close) = params.find(|c: char| {
        depth += match c {
            '(' => 1,
            ')' => -1,
            _ => 0,
        };
        depth == 0
    }) else {
        return false;
    };
    let receiver = params[..close].split(',').next().unwrap_or("").trim();
    let returns = params[close + 1..].trim_start().strip_prefix("->").unwrap_or("");
    let returns = returns.split(['{', ';']).next().unwrap_or("").split(" where").next().unwrap_or("");
    matches!(receiver, "self" | "mut self") && returns.trim() == "Self"
}

/// Where a setter is declared: an `impl` of a named type, or a macro body that names its type `$x`.
#[derive(Debug, Clone, PartialEq)]
enum Owner {
    Type(String),
    Macro(String),
    Unknown,
}

/// Every `pub fn` in `source` that takes `self` by value and returns `Self`, with its owner, its name and whether an
/// attribute or doc line above it holds `#[must_use`.
fn setters(source: &str) -> Vec<(Owner, String, bool)> {
    let lines: Vec<&str> = source.lines().map(str::trim).collect();
    let (mut found, mut owner, mut current_macro) = (Vec::new(), Owner::Unknown, String::new());
    for (i, line) in lines.iter().enumerate() {
        if let Some(rest) = line.strip_prefix("macro_rules!") {
            current_macro = ident(rest).to_owned();
            owner = Owner::Unknown;
        } else if let Some(rest) = line.strip_prefix("impl").filter(|r| r.starts_with([' ', '<'])) {
            let rest = skip_generics(rest);
            owner = match ident(rest) {
                _ if rest.contains(" for ") => Owner::Unknown,
                name if name.starts_with('$') => Owner::Macro(current_macro.clone()),
                "" => Owner::Unknown,
                name => Owner::Type(name.to_owned()),
            };
        }
        let Some(rest) = line.strip_prefix("pub fn ").or_else(|| line.strip_prefix("pub const fn ")) else { continue };
        let end = lines[i..].iter().position(|l| l.contains('{') || l.ends_with(';')).map_or(lines.len(), |e| i + e);
        let signature = std::iter::once(rest).chain(lines[i + 1..=end.min(lines.len() - 1)].iter().copied());
        if !is_chainable(&signature.collect::<Vec<_>>().join(" ")) {
            continue;
        }
        let above = lines[..i].iter().rev().take_while(|l| l.starts_with("#[") || l.starts_with("//"));
        let must_use = above.into_iter().any(|l| l.starts_with("#[must_use"));
        found.push((owner.clone(), ident(rest).to_owned(), must_use));
    }
    found
}

/// The types each macro is invoked with as `name!(Type…`; invocations that pass on a `$x` are left out.
fn macro_types(sources: &[String], name: &str) -> Vec<String> {
    let invocation = format!("{name}!(");
    let named =
        sources.iter().flat_map(|s| s.match_indices(&invocation).map(|(at, _)| ident(&s[at + invocation.len()..])));
    named.filter(|n| !n.is_empty() && !n.starts_with('$')).map(str::to_owned).collect()
}

/// The chainable setters of `sources`, and those that can be dropped silently: neither they nor their type are
/// `#[must_use]`, and the type isn't in `SIDE_EFFECT_SETTERS`. A setter in a macro body is covered when every type
/// the macro is invoked with is `#[must_use]`.
fn unmarked_setters(sources: &[String]) -> (Vec<(Owner, String, bool)>, Vec<String>) {
    let mut types = BTreeMap::new();
    sources.iter().for_each(|s| declarations(s, &mut types));
    let must_use = |t: &str| types.get(t) == Some(&true);
    let all: Vec<_> = sources.iter().flat_map(|s| setters(s)).collect();
    let mut macros = BTreeMap::new();
    let mut covered = |owner: &Owner| match owner {
        Owner::Type(t) => must_use(t) || SIDE_EFFECT_SETTERS.contains(&t.as_str()),
        Owner::Macro(m) => *macros.entry(m.clone()).or_insert_with(|| {
            let invoked = macro_types(sources, m);
            !invoked.is_empty() && invoked.iter().all(|t| must_use(t))
        }),
        Owner::Unknown => false,
    };
    let missing = all.iter().filter(|(owner, _, marked)| !marked && !covered(owner));
    let names = missing.map(|(owner, name, _)| match owner {
        Owner::Type(t) => format!("{t}::{name}"),
        Owner::Macro(m) => format!("{m}!::{name}"),
        Owner::Unknown => format!("?::{name}"),
    });
    let names = names.collect();
    (all, names)
}

#[test]
fn every_request_builder_is_must_use() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let (mut names, mut types) = (BTreeSet::new(), BTreeMap::new());
    for source in &files {
        builders(source, &mut names);
        declarations(source, &mut types);
    }
    // A scan that finds almost nothing has stopped matching the macros, not proven anything.
    assert!(names.len() > 100, "found only {} request builders", names.len());
    let undeclared: Vec<_> = names.iter().filter(|n| !types.contains_key(*n)).collect();
    let missing: Vec<_> = names.iter().filter(|n| types.get(*n) == Some(&false)).collect();
    assert!(undeclared.is_empty(), "request builders whose declaration the scan can't find: {undeclared:?}");
    assert!(missing.is_empty(), "request builders without #[must_use]: {missing:?}");
}

#[test]
fn every_pager_is_awaitable() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let (mut awaitable, mut pager_types) = (BTreeSet::new(), BTreeSet::new());
    for source in &files {
        builders(source, &mut awaitable);
        pagers(source, &mut pager_types);
    }
    assert!(pager_types.len() > 30, "found only {} pagers", pager_types.len());
    let stale: Vec<_> = STREAMS.iter().filter(|s| !pager_types.contains(**s)).collect();
    assert!(stale.is_empty(), "STREAMS names types with no `next`: {stale:?}");
    let not_awaitable: Vec<_> =
        pager_types.iter().filter(|p| !awaitable.contains(*p) && !STREAMS.contains(&p.as_str())).collect();
    assert!(not_awaitable.is_empty(), "pagers that can't be awaited: {not_awaitable:?}");
}

#[test]
fn every_chainable_setter_is_must_use() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let (found, missing) = unmarked_setters(&files);
    assert!(found.len() > 500, "found only {} chainable setters", found.len());
    let stale: Vec<_> = SIDE_EFFECT_SETTERS
        .iter()
        .filter(|t| !found.iter().any(|(owner, _, _)| *owner == Owner::Type(t.to_string())))
        .collect();
    assert!(stale.is_empty(), "SIDE_EFFECT_SETTERS names types with no chainable setter: {stale:?}");
    assert!(missing.is_empty(), "chainable setters without #[must_use] on them or their type: {missing:?}");
}

#[test]
fn setter_scan_catches_planted_cases() {
    let planted = r#"
pub struct Plain { x: i32 }

#[must_use = "a request does nothing until it is awaited"]
pub struct Request<'a> { x: &'a i32 }

impl Plain {
    /// Unmarked.
    pub fn bare(mut self, x: i32) -> Self {
        self
    }

    pub fn wrapped<T: Into<Vec<u8>>>(
        mut self,
        value: T,
    ) -> Self
    where
        T: Clone,
    {
        self
    }

    pub const fn by_value(self) -> Self {
        self
    }

    #[must_use]
    /// Marked, with a doc line in between.
    pub fn marked(self, pair: (i32, i32)) -> Self {
        self
    }

    pub fn new() -> Self {
        Plain { x: 0 }
    }

    pub fn by_ref(&mut self) -> &mut Self {
        self
    }

    pub fn other(self) -> Option<Self> {
        None
    }

    pub(crate) fn internal(self) -> Self {
        self
    }
}

impl<'a> Request<'a> {
    pub fn covered(mut self, x: impl Into<String>) -> Self {
        self
    }
}

impl Clone for Plain {
    fn clone(&self) -> Self {
        Plain { x: self.x }
    }
}

macro_rules! plain_setters {
    ($t:ident) => {
        impl $t {
            pub fn from_macro(self) -> Self {
                self
            }
        }
    };
}
plain_setters!(Plain);

macro_rules! request_setters {
    ($t:ident) => {
        impl<T: $crate::Bound<Vec<u8>>> $t<'_, T> {
            pub fn covered_by_type(self) -> Self {
                self
            }
        }
    };
}
request_setters!(Request);

macro_rules! unused_setters {
    ($t:ident) => {
        impl $t {
            pub fn never_invoked(self) -> Self {
                self
            }
        }
    };
}
"#;
    let (found, missing) = unmarked_setters(&[planted.to_owned()]);
    assert_eq!(found.len(), 8);
    let expected = [
        "Plain::bare",
        "Plain::wrapped",
        "Plain::by_value",
        "plain_setters!::from_macro",
        "unused_setters!::never_invoked",
    ];
    assert_eq!(missing, expected);
}
