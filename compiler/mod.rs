use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;

struct Field {
    name: String,
    flag: Option<(String, u32)>,
    ty: String,
}

struct Def {
    name: String,
    id: u32,
    generic: bool,
    fields: Vec<Field>,
    ty: String,
    function: bool,
}

/// `string_is_bytes`: mtproto.tl writes binary data (`pq`, `g_a`, `encrypted_data`, ...) as
/// `string`; it has the same wire format as `bytes`, but must not be decoded as UTF-8.
fn parse(src: &str, string_is_bytes: bool) -> Vec<Def> {
    let mut defs = Vec::new();
    let mut function = false;
    for line in src.lines() {
        let line = line.split("//").next().unwrap().trim();
        match line {
            "---functions---" => function = true,
            "---types---" => function = false,
            _ => defs.extend(line.strip_suffix(';').and_then(|d| parse_def(d, function))),
        }
    }
    if string_is_bytes {
        for f in defs.iter_mut().flat_map(|d| &mut d.fields).filter(|f| f.ty == "string") {
            f.ty = "bytes".into();
        }
    }
    defs
}

fn parse_def(d: &str, function: bool) -> Option<Def> {
    let (head, ty) = d.split_once(" = ")?;
    let ty = ty.trim();
    if !function && (matches!(ty, "Bool" | "True") || ty.starts_with("Vector")) {
        return None;
    }
    let mut tokens = head.split_whitespace();
    let (name, id) = tokens.next()?.split_once('#')?;
    let mut def = Def {
        name: name.into(),
        id: u32::from_str_radix(id, 16).ok()?,
        generic: false,
        fields: Vec::new(),
        ty: ty.into(),
        function,
    };
    for t in tokens {
        if t == "{X:Type}" {
            def.generic = true;
            continue;
        }
        let (name, ty) = t.split_once(':')?;
        let (flag, ty) = match ty.split_once('?') {
            Some((cond, ty)) => {
                let (flags, bit) = cond.split_once('.')?;
                (Some((flags.into(), bit.parse().ok()?)), ty)
            }
            None => (None, ty),
        };
        def.fields.push(Field { name: name.into(), flag, ty: ty.trim_start_matches('!').into() });
    }
    Some(def)
}

fn split_ns(name: &str) -> (&str, &str) {
    name.rsplit_once('.').unwrap_or(("", name))
}

fn pascal(s: &str) -> String {
    s.split('_')
        .map(|p| {
            let mut c = p.chars();
            c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
        })
        .collect()
}

fn path(kind: &str, tl_name: &str) -> String {
    match split_ns(tl_name) {
        ("", n) => format!("crate::raw::{kind}::{}", pascal(n)),
        (ns, n) => format!("crate::raw::{kind}::{ns}::{}", pascal(n)),
    }
}

fn ident(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    match n.as_str() {
        "self" => "is_self".into(),
        "as" | "break" | "const" | "continue" | "else" | "enum" | "extern" | "false" | "fn" | "for" | "if" | "impl"
        | "in" | "let" | "loop" | "match" | "mod" | "move" | "mut" | "pub" | "ref" | "return" | "static" | "struct"
        | "trait" | "true" | "type" | "unsafe" | "use" | "where" | "while" | "async" | "await" | "dyn" | "abstract"
        | "become" | "box" | "do" | "final" | "macro" | "override" | "priv" | "typeof" | "unsized" | "virtual"
        | "yield" | "try" | "gen" => format!("r#{n}"),
        _ => n,
    }
}

fn vector_inner(ty: &str) -> Option<&str> {
    ty.strip_prefix("Vector<")?.strip_suffix('>')
}

fn rust_ty(ty: &str, generic: bool) -> String {
    match ty {
        "int" => "i32".into(),
        "long" => "i64".into(),
        "double" => "f64".into(),
        "string" => "::std::string::String".into(),
        "bytes" => "::std::vec::Vec<u8>".into(),
        "int128" => "[u8; 16]".into(),
        "int256" => "[u8; 32]".into(),
        "Bool" | "true" => "bool".into(),
        "X" if generic => "X".into(),
        _ => match vector_inner(ty) {
            Some(inner) => format!("::std::vec::Vec<{}>", rust_ty(inner, generic)),
            None => path("base", ty),
        },
    }
}

fn resolves(ty: &str, generic: bool, types: &HashSet<String>) -> bool {
    match ty {
        "#" | "int" | "long" | "double" | "string" | "bytes" | "int128" | "int256" | "Bool" | "true" => true,
        "X" => generic,
        _ => match vector_inner(ty) {
            Some(inner) => resolves(inner, generic, types),
            None => types.contains(ty),
        },
    }
}

fn field_ty(f: &Field, generic: bool) -> String {
    let t = rust_ty(&f.ty, generic);
    match f.flag {
        Some(_) if f.ty != "true" => format!("::std::option::Option<{t}>"),
        _ => t,
    }
}

fn gen_struct(out: &mut String, d: &Def) {
    let name = pascal(split_ns(&d.name).1);
    let (g, g_ser) = if d.generic { ("<X>", "<X: crate::raw::Serializable>") } else { ("", "") };
    let id = d.id;

    writeln!(out, "#[derive(Debug, Clone, PartialEq)]\npub struct {name}{g} {{").unwrap();
    for f in d.fields.iter().filter(|f| f.ty != "#") {
        writeln!(out, "    pub {}: {},", ident(&f.name), field_ty(f, d.generic)).unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out, "impl{g} crate::raw::Identifiable for {name}{g} {{ const CONSTRUCTOR_ID: u32 = {id:#010x}; }}")
        .unwrap();

    writeln!(out, "impl{g_ser} crate::raw::Serializable for {name}{g} {{").unwrap();
    writeln!(out, "    fn serialize(&self, buf: &mut ::std::vec::Vec<u8>) {{").unwrap();
    writeln!(out, "        crate::raw::Serializable::serialize(&{id:#010x}u32, buf);").unwrap();
    for f in &d.fields {
        let field = ident(&f.name);
        if f.ty == "#" {
            let bits: Vec<String> = d
                .fields
                .iter()
                .filter(|c| c.flag.as_ref().is_some_and(|(fl, _)| *fl == f.name))
                .map(|c| {
                    let (mask, cf) = (1u32 << c.flag.as_ref().unwrap().1, ident(&c.name));
                    let set = if c.ty == "true" { format!("self.{cf}") } else { format!("self.{cf}.is_some()") };
                    format!("if {set} {{ {mask:#x}u32 }} else {{ 0 }}")
                })
                .collect();
            let bits = if bits.is_empty() { "0u32".into() } else { bits.join(" | ") };
            writeln!(out, "        crate::raw::Serializable::serialize(&({bits}), buf);").unwrap();
        } else if f.flag.is_some() && f.ty == "true" {
        } else if f.flag.is_some() {
            writeln!(out, "        if let Some(v) = &self.{field} {{ crate::raw::Serializable::serialize(v, buf); }}")
                .unwrap();
        } else {
            writeln!(out, "        crate::raw::Serializable::serialize(&self.{field}, buf);").unwrap();
        }
    }
    writeln!(out, "    }}\n}}").unwrap();
}

fn gen_deserialize_bare(out: &mut String, d: &Def) {
    let name = pascal(split_ns(&d.name).1);
    writeln!(out, "impl {name} {{").unwrap();
    writeln!(out, "    #[doc(hidden)]").unwrap();
    writeln!(out, "    pub fn deserialize_bare(buf: &mut &[u8]) -> ::std::result::Result<Self, crate::raw::Error> {{")
        .unwrap();
    let mut names = Vec::new();
    for f in &d.fields {
        let read = |ty: &str| format!("<{} as crate::raw::Deserializable>::deserialize(buf)?", rust_ty(ty, false));
        match &f.flag {
            _ if f.ty == "#" => {
                writeln!(out, "        let __{} = <u32 as crate::raw::Deserializable>::deserialize(buf)?;", f.name)
                    .unwrap();
                continue;
            }
            Some((flags, bit)) if f.ty == "true" => {
                writeln!(out, "        let {} = (__{flags} & {:#x}) != 0;", ident(&f.name), 1u32 << bit).unwrap();
            }
            Some((flags, bit)) => {
                writeln!(
                    out,
                    "        let {} = if (__{flags} & {:#x}) != 0 {{ Some({}) }} else {{ None }};",
                    ident(&f.name),
                    1u32 << bit,
                    read(&f.ty)
                )
                .unwrap();
            }
            None => writeln!(out, "        let {} = {};", ident(&f.name), read(&f.ty)).unwrap(),
        }
        names.push(ident(&f.name));
    }
    writeln!(out, "        Ok(Self {{ {} }})\n    }}\n}}", names.join(", ")).unwrap();
}

fn gen_enum(out: &mut String, ty: &str, ctors: &[&Def]) {
    let name = pascal(split_ns(ty).1);
    let variants: Vec<String> = ctors.iter().map(|c| pascal(split_ns(&c.name).1)).collect();
    let unique: HashSet<_> = variants.iter().collect();
    assert_eq!(unique.len(), variants.len(), "two constructors of {ty} share a name");

    writeln!(out, "#[derive(Debug, Clone, PartialEq)]\npub enum {name} {{").unwrap();
    for (c, v) in ctors.iter().zip(&variants) {
        if c.fields.is_empty() {
            writeln!(out, "    {v},").unwrap();
        } else {
            writeln!(out, "    {v}(::std::boxed::Box<{}>),", path("types", &c.name)).unwrap();
        }
    }
    writeln!(out, "}}").unwrap();

    writeln!(out, "impl crate::raw::Serializable for {name} {{").unwrap();
    writeln!(out, "    fn serialize(&self, buf: &mut ::std::vec::Vec<u8>) {{\n        match self {{").unwrap();
    for (c, v) in ctors.iter().zip(&variants) {
        if c.fields.is_empty() {
            writeln!(out, "            Self::{v} => crate::raw::Serializable::serialize(&{:#010x}u32, buf),", c.id)
                .unwrap();
        } else {
            writeln!(out, "            Self::{v}(x) => crate::raw::Serializable::serialize(&**x, buf),").unwrap();
        }
    }
    writeln!(out, "        }}\n    }}\n}}").unwrap();

    writeln!(out, "impl crate::raw::Deserializable for {name} {{").unwrap();
    writeln!(out, "    fn deserialize(buf: &mut &[u8]) -> ::std::result::Result<Self, crate::raw::Error> {{").unwrap();
    writeln!(out, "        let id = <u32 as crate::raw::Deserializable>::deserialize(buf)?;\n        Ok(match id {{")
        .unwrap();
    for (c, v) in ctors.iter().zip(&variants) {
        if c.fields.is_empty() {
            writeln!(out, "            {:#010x} => Self::{v},", c.id).unwrap();
        } else {
            writeln!(
                out,
                "            {:#010x} => Self::{v}(::std::boxed::Box::new({}::deserialize_bare(buf)?)),",
                c.id,
                path("types", &c.name)
            )
            .unwrap();
        }
    }
    writeln!(out, "            _ => return Err(crate::raw::Error::UnexpectedConstructor {{ id }}),").unwrap();
    writeln!(out, "        }})\n    }}\n}}").unwrap();
}

fn gen_counter(out: &mut String, ctors: &[&Def], name: &str, ty: &str, fields: &[&str]) {
    let has = |c: &Def, n: &str| c.fields.iter().any(|f| f.name == n && f.ty == "int" && f.flag.is_none());
    let value = fields.iter().map(|f| format!("u.{f}")).collect::<Vec<_>>().join(", ");
    let value = if fields.len() > 1 { format!("({value})") } else { value };
    writeln!(out, "    pub fn {name}(&self) -> ::std::option::Option<{ty}> {{\n        match self {{").unwrap();
    for c in ctors.iter().filter(|c| fields.iter().all(|f| has(c, f))) {
        writeln!(out, "            Self::{}(u) => Some({value}),", pascal(split_ns(&c.name).1)).unwrap();
    }
    writeln!(out, "            _ => None,\n        }}\n    }}").unwrap();
}

fn gen_update_counters(out: &mut String, ctors: &[&Def]) {
    writeln!(out, "impl Update {{").unwrap();
    gen_counter(out, ctors, "pts", "(i32, i32)", &["pts", "pts_count"]);
    gen_counter(out, ctors, "qts", "i32", &["qts"]);
    writeln!(out, "}}").unwrap();
}

fn gen_function(out: &mut String, d: &Def) {
    gen_struct(out, d);
    let name = pascal(split_ns(&d.name).1);
    if d.generic {
        writeln!(
            out,
            "impl<X: crate::raw::RemoteCall> crate::raw::RemoteCall for {name}<X> {{ type Return = X::Return; }}"
        )
        .unwrap();
    } else {
        writeln!(out, "impl crate::raw::RemoteCall for {name} {{ type Return = {}; }}", rust_ty(&d.ty, false)).unwrap();
    }
}

pub fn generate(api: &str, mtproto: &str) -> String {
    let layer: i32 = api
        .lines()
        .find_map(|l| l.strip_prefix("// LAYER "))
        .and_then(|n| n.trim().parse().ok())
        .expect("no `// LAYER n` line in api.tl");

    let mut defs = parse(mtproto, true);
    defs.extend(parse(api, false));

    loop {
        let types: HashSet<String> = defs.iter().filter(|d| !d.function).map(|d| d.ty.clone()).collect();
        let before = defs.len();
        defs.retain(|d| {
            d.fields.iter().all(|f| resolves(&f.ty, d.generic, &types))
                && (!d.function || d.generic || resolves(&d.ty, false, &types))
        });
        if defs.len() == before {
            break;
        }
    }

    let mut mods: BTreeMap<(&str, &str), String> = BTreeMap::new();
    let mut by_type: BTreeMap<&str, Vec<&Def>> = BTreeMap::new();
    for d in &defs {
        let ns = split_ns(&d.name).0;
        if d.function {
            gen_function(mods.entry(("functions", ns)).or_default(), d);
        } else {
            by_type.entry(&d.ty).or_default().push(d);
            if !d.fields.is_empty() {
                let out = mods.entry(("types", ns)).or_default();
                gen_struct(out, d);
                gen_deserialize_bare(out, d);
            }
        }
    }
    for (ty, ctors) in &by_type {
        let out = mods.entry(("base", split_ns(ty).0)).or_default();
        gen_enum(out, ty, ctors);
        if *ty == "Update" {
            gen_update_counters(out, ctors);
        }
    }

    let mut out = format!("/// The API layer this code was generated from.\npub const LAYER: i32 = {layer};\n");
    for kind in ["types", "base", "functions"] {
        writeln!(out, "pub mod {kind} {{").unwrap();
        for ((k, ns), code) in &mods {
            match (*k == kind, *ns) {
                (false, _) => {}
                (true, "") => out.push_str(code),
                (true, ns) => writeln!(out, "pub mod {ns} {{\n{code}}}").unwrap(),
            }
        }
        writeln!(out, "}}").unwrap();
    }
    out
}
