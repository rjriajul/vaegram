//! The friendly API takes and returns friendly types only: a TL type (`raw::base::*`, `raw::types::*`) in a public
//! signature would tie every caller to the TL schema of one layer. This scans `src/` for TL paths in
//! `pub fn` signatures, `pub` fields, `pub enum` variants, `IntoFuture` outputs and `From`/`TryFrom` impls.
//!
//! Allowed without an entry here:
//! - `src/raw/`: the TL layer itself, reached through `Client::invoke`.
//! - Items gated by `#[cfg(feature = "unstable-raw")]`, on the item or on its enclosing `impl`.
//! - `pub(crate)`/`pub(super)` items, and `#[cfg(test)]` modules.
//!
//! The scan is line-based: it reads a signature up to its body and doesn't resolve types, so a TL type renamed on
//! import (`use crate::raw::base::User as RawUser`) slips through.

use std::fs;
use std::path::Path;

/// `(file, item)` pairs that may name TL types, and why.
const ALLOWED: &[(&str, &str)] = &[
    // The chat id converters between Bot API ids and TL peers, for code built on `invoke`.
    ("src/utils.rs", "get_peer_id"),
    ("src/utils.rs", "get_peer"),
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
    let source = r#"use crate::raw::{base, types};
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

impl From<base::InputFileLocation> for Friendly {}

impl IntoFuture for Fetch<'_> {
    type Output = Result<base::Updates>;
}

into_future!(Upload, base::InputFile);
into_future!(Hidden, base::InputFile);

#[cfg(test)]
mod tests {
    pub fn helper() -> base::User {}
}
"#;
    let items: Vec<String> = violations(source).into_iter().map(|(_, item, _)| item).collect();
    assert_eq!(items, ["raw", "Wrapper", "Raw", "Alias", "leaks", "Friendly", "Fetch", "Upload"]);
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

/// `(line, item, text)` for each public signature in `source` that names a TL type. Items of types the file
/// declares crate-private (`struct`, `pub(crate) struct`, …) are skipped: nothing outside the crate sees them.
fn violations(source: &str) -> Vec<(usize, String, String)> {
    let lines: Vec<&str> = source.lines().collect();
    let tl = TlNames::of(source);
    let private = private_types(&lines);
    let is_private = |name: &str| private.iter().any(|p| p == name);
    let mut found = Vec::new();
    let mut attrs = String::new();
    // The top-level item the current line is in: its type name, and whether it's gated or private.
    let mut owner = String::new();
    let mut hidden_owner = false;
    let mut pub_enum: Option<usize> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();
        if trimmed == "#[cfg(test)]" && lines.get(i + 1).is_some_and(|l| l.trim().starts_with("mod ")) {
            i = if lines[i + 1].trim_end().ends_with('{') { end_of_block(&lines, i + 1, indent) + 1 } else { i + 2 };
            attrs.clear();
            continue;
        }
        if trimmed.starts_with("#[") {
            attrs.push_str(trimmed);
            i += 1;
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with("//") {
            i += 1;
            continue;
        }
        let gated = attrs.contains("cfg(feature = \"unstable-raw\")") || attrs.contains("cfg(test)");
        attrs.clear();
        if indent == 0 && trimmed != "}" {
            owner = owner_of(trimmed).unwrap_or_default();
            hidden_owner = gated || is_private(&owner);
        }
        let report = |found: &mut Vec<_>, item: &str, text: &str| {
            if tl.found_in(text) && !gated && !hidden_owner {
                found.push((i + 1, item.to_string(), text.to_string()));
            }
        };

        if let Some(start) = pub_enum {
            if trimmed == "}" && indent == start {
                pub_enum = None;
            } else {
                report(&mut found, trimmed.split(['(', ' ', '{', ',']).next().unwrap_or(trimmed), trimmed);
            }
            i += 1;
            continue;
        }
        if trimmed.starts_with("pub enum ") && trimmed.ends_with('{') {
            pub_enum = Some(indent);
        } else if let Some(name) = fn_name(trimmed) {
            let mut end = i;
            while !lines[end].contains('{') && !lines[end].trim_end().ends_with(';') && end + 1 < lines.len() {
                end += 1;
            }
            let signature = lines[i..=end].iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ");
            report(&mut found, name, signature.split('{').next().unwrap_or(&signature));
            i = end;
        } else if let Some(name) = pub_field(trimmed) {
            report(&mut found, name, trimmed);
        } else if trimmed.starts_with("type Output") || trimmed.starts_with("impl From<") {
            report(&mut found, &owner, trimmed.split('{').next().unwrap_or(trimmed));
        } else if let Some(args) = trimmed.strip_prefix("into_future!(") {
            let name = args.split(',').next().unwrap_or(args);
            if !is_private(name) {
                report(&mut found, name, trimmed);
            }
        }
        i += 1;
    }
    found
}

/// The type a top-level line declares or implements for: `Foo` in `pub struct Foo {`, `impl<'a> Foo<'a> {` and
/// `impl From<X> for Foo {`.
fn owner_of(line: &str) -> Option<String> {
    let rest = if line.starts_with("impl") {
        let head = line.split('{').next().unwrap_or(line);
        match head.rsplit_once(" for ") {
            Some((_, target)) => target,
            None => head.strip_prefix("impl")?.trim_start_matches(|c| c != ' ').trim_start(),
        }
    } else {
        let line = ["pub(crate) ", "pub(super) ", "pub "].iter().fold(line, |l, p| l.strip_prefix(p).unwrap_or(l));
        ["struct ", "enum "].iter().find_map(|kind| line.strip_prefix(kind))?
    };
    let name: String = rest.trim().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    (!name.is_empty()).then_some(name)
}

/// Types declared without `pub`, or with a restricted `pub(…)`.
fn private_types(lines: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    for line in lines.iter().map(|l| l.trim()) {
        if line.starts_with("pub ") {
            continue;
        }
        let rest = line.strip_prefix("pub(crate) ").or_else(|| line.strip_prefix("pub(super) ")).unwrap_or(line);
        if let Some(decl) = ["struct ", "enum "].iter().find_map(|kind| rest.strip_prefix(kind)) {
            names.push(decl.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect());
        }
    }
    names
}

/// The name of a `pub fn` (not `pub(crate)`), if the line starts one.
fn fn_name(line: &str) -> Option<&str> {
    let mut rest = line.strip_prefix("pub ")?;
    for qualifier in ["const ", "async ", "unsafe "] {
        rest = rest.strip_prefix(qualifier).unwrap_or(rest);
    }
    let rest = rest.strip_prefix("fn ")?;
    rest.split(['(', '<']).next()
}

/// The name of a `pub` field, tuple struct, type alias, const or static, if the line declares one.
fn pub_field(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("pub ")?;
    let ident = |s: &str| s.chars().all(|c| c.is_alphanumeric() || c == '_');
    if let Some(rest) = rest.strip_prefix("struct ") {
        return rest.contains("(pub ").then(|| rest.split(['(', '<']).next().unwrap_or(rest));
    }
    if let Some(rest) = rest.strip_prefix("type ") {
        return rest.split([' ', '<', '=']).next();
    }
    let rest = rest.strip_prefix("const ").or_else(|| rest.strip_prefix("static ")).unwrap_or(rest);
    let (name, _) = rest.split_once(':')?;
    ident(name).then_some(name)
}

/// The ways a file can spell a TL type: `base::` and `types::` when it imports them from `crate::raw`, `raw::base::`
/// and `raw::types::` anywhere, and names it imports straight from them.
struct TlNames {
    modules: Vec<&'static str>,
    names: Vec<String>,
}

impl TlNames {
    fn of(source: &str) -> TlNames {
        let mut modules = Vec::new();
        let mut names = Vec::new();
        for statement in source.split(';') {
            let statement = statement.split_whitespace().collect::<Vec<_>>().join(" ");
            let Some(at) = statement.find("use crate::raw::") else { continue };
            let path = statement[at + "use crate::raw::".len()..].replace("{ ", "{").replace(" }", "}");
            let path = path.as_str();
            if let Some(group) = path.strip_prefix('{').and_then(|g| g.strip_suffix('}')) {
                for module in ["base", "types"] {
                    if group.split(',').any(|part| part.trim() == module) {
                        modules.push(module);
                    }
                }
                continue;
            }
            for module in ["base", "types"] {
                if path == module {
                    modules.push(module);
                } else if let Some(imported) = path.strip_prefix(module).and_then(|p| p.strip_prefix("::")) {
                    let list = imported.trim_start_matches('{').trim_end_matches('}');
                    for part in list.split(',') {
                        let name = part.trim().rsplit("::").next().unwrap_or("");
                        if !name.is_empty() {
                            names.push(name.to_string());
                        }
                    }
                }
            }
        }
        TlNames { modules, names }
    }

    fn found_in(&self, text: &str) -> bool {
        if text.contains("raw::base::") || text.contains("raw::types::") {
            return true;
        }
        let bare = |at: usize| at == 0 || !text[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == ':');
        let modules = self.modules.iter().any(|module| {
            let path = format!("{module}::");
            text.match_indices(&path).any(|(at, _)| bare(at))
        });
        modules
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
