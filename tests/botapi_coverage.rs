use std::collections::HashSet;
use std::fs;
use std::path::Path;

const SPEC: &str = include_str!("../tools/botapi/botapi.txt");
const MAP: &str = include_str!("../tools/botapi/methods.txt");

enum Status<'a> {
    Done(&'a str),
    Todo(&'a str),
    NotApplicable,
}

fn mapping(text: &str) -> Vec<(&str, Status<'_>)> {
    text.lines().filter(|l| !l.is_empty() && !l.starts_with('#')).map(entry).collect()
}

fn entry(line: &str) -> (&str, Status<'_>) {
    let (name, to) = line.split_once(' ').unwrap_or_else(|| panic!("methods.txt: `{line}` maps to nothing"));
    let to = to.trim();
    let status = match (to.strip_prefix("n/a:"), to.strip_prefix("todo ")) {
        (Some(reason), _) if reason.trim().is_empty() => panic!("methods.txt: {name} is n/a without a reason"),
        (Some(_), _) => Status::NotApplicable,
        (_, Some(method)) => Status::Todo(method.trim()),
        _ => Status::Done(to),
    };
    (name, status)
}

// Client methods take `&self`; builder setters take `self`, so they don't count.
fn client_methods(dir: &Path, names: &mut HashSet<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            client_methods(&path, names);
            continue;
        }
        for item in fs::read_to_string(&path).unwrap().split("pub ").skip(1) {
            let Some(item) = item.strip_prefix("async ").unwrap_or(item).strip_prefix("fn ") else { continue };
            let Some((name, params)) = item.split_once('(') else { continue };
            if params.trim_start().starts_with('&') {
                names.insert(name.split_once('<').map_or(name, |(n, _)| n).to_owned());
            }
        }
    }
}

#[test]
fn every_bot_api_method_is_mapped_once() {
    let methods: Vec<_> = SPEC.lines().filter_map(|l| l.strip_prefix("method ")).collect();
    let mapped = mapping(MAP);

    let mut seen = HashSet::new();
    let twice: Vec<_> = mapped.iter().map(|(n, _)| *n).filter(|n| !seen.insert(*n)).collect();
    let unmapped: Vec<_> = methods.iter().filter(|n| !seen.contains(*n)).collect();
    let unknown: Vec<_> = seen.iter().filter(|n| !methods.contains(*n)).collect();
    assert!(
        twice.is_empty() && unmapped.is_empty() && unknown.is_empty(),
        "tools/botapi/methods.txt is out of date\n  unmapped: {unmapped:?}\n  not in the Bot API: {unknown:?}\n  \
         mapped twice: {twice:?}"
    );
}

#[test]
fn mapped_methods_exist() {
    let mut names = HashSet::new();
    client_methods(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src/methods"), &mut names);

    let (mut done, mut todo, mut na) = (0, 0, 0);
    let (mut missing, mut landed) = (Vec::new(), Vec::new());
    for (bot, status) in mapping(MAP) {
        match status {
            Status::Done(m) if names.contains(m) => done += 1,
            Status::Done(m) => missing.push(format!("{bot} -> {m}")),
            Status::Todo(m) if names.contains(m) => {
                done += 1;
                landed.push(bot);
            }
            Status::Todo(_) => todo += 1,
            Status::NotApplicable => na += 1,
        }
    }

    let version = SPEC.lines().next().and_then(|l| l.strip_prefix("# ")?.split_once(',')).map_or("Bot API", |v| v.0);
    let applicable = done + todo + missing.len();
    let percent = 100.0 * done as f64 / applicable as f64;
    println!("{version}: {done}/{applicable} methods ({percent:.1}%), {todo} todo, {na} n/a");
    if !landed.is_empty() {
        println!("merged but still marked todo in methods.txt: {landed:?}");
    }
    assert!(missing.is_empty(), "mapped methods are gone from src/methods: {missing:?}");
}
