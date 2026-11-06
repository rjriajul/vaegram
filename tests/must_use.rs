use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// Macros that implement `IntoFuture` for the type named as their first argument.
const BUILDER_MACROS: [&str; 4] = ["into_future!(", "send_builder!(", "send_media!(", "pager!("];

/// Types with a `next` that are not pagers: a chunk stream, whose file `DownloadMedia` already collects when awaited.
const STREAMS: [&str; 1] = ["MediaStream"];

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
    let not_awaitable: Vec<_> =
        pager_types.iter().filter(|p| !awaitable.contains(*p) && !STREAMS.contains(&p.as_str())).collect();
    assert!(not_awaitable.is_empty(), "pagers that can't be awaited: {not_awaitable:?}");
}
