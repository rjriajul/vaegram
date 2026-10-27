use std::collections::{BTreeMap, HashMap, HashSet};
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

// Types whose every value is a credential: bot tokens, SRP proofs, payment credentials, Passport data and
// secrets, wallet recovery phrases, email login tokens and call relay passwords.
const SECRET_TYPES: [&str; 14] = [
    "bots.ExportedBotToken",
    "InputCheckPasswordSRP",
    "InputPaymentCredentials",
    "SecureSecretSettings",
    "SecureValue",
    "InputSecureValue",
    "SecureData",
    "SecureFile",
    "InputSecureFile",
    "SecureCredentialsEncrypted",
    "wallet.SecretPhraseParts",
    "wallet.EncryptedSecretPhrasePart",
    "EmailVerification",
    "PhoneConnection",
];
// Fields that are credentials wherever they appear.
const SECRET_FIELDS: [&str; 4] = ["tmp_password", "phone_code_hash", "match_code", "match_codes"];

/// Logins, codes, passwords, recovery, takeout and every definition that carries or returns a credential:
/// everything `storage::recording` must not keep. A request is cut when it carries one or is answered with one,
/// so its answer is cut too.
fn credential(d: &Def) -> bool {
    let (ns, name) = split_ns(&d.name);
    let name = name.to_ascii_lowercase();
    // Every auth.* type holds a login or a step of one, wherever it appears (updateSentPhoneCode carries
    // auth.SentCode).
    let secret_ty = |mut ty: &str| {
        while let Some(inner) = vector_inner(ty) {
            ty = inner;
        }
        SECRET_TYPES.contains(&ty) || split_ns(ty).0 == "auth"
    };
    ns == "auth"
        || (ns == "account" && ["password", "email", "takeout", "phone", "device"].iter().any(|w| name.contains(w)))
        || matches!(name.as_str(), "invokewithapnssecret" | "invokewithgoogleplayintegrity" | "invokewithrecaptcha")
        || secret_ty(&d.ty)
        || d.fields.iter().any(|f| secret_ty(&f.ty) || SECRET_FIELDS.contains(&f.name.as_str()))
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

fn gen_struct_json(out: &mut String, d: &Def) {
    let name = pascal(split_ns(&d.name).1);
    let (g, g_json) = if d.generic { ("<X>", "<X: crate::raw::ToJson>") } else { ("", "") };
    writeln!(out, "impl{g_json} crate::raw::ToJson for {name}{g} {{").unwrap();
    writeln!(out, "    fn to_json(&self, out: &mut ::std::string::String) {{").unwrap();
    writeln!(out, "        out.push_str(r#\"{{\"_\":\"{}\"\"#);", d.name).unwrap();
    for f in d.fields.iter().filter(|f| f.ty != "#") {
        let (key, field) = (format!("r#\",\"{}\":\"#", f.name), ident(&f.name));
        let write = "{ out.push_str(KEY); crate::raw::ToJson::to_json(v, out); }".replace("KEY", &key);
        if f.flag.is_some() && f.ty != "true" {
            writeln!(out, "        if let Some(v) = &self.{field} {write}").unwrap();
        } else {
            writeln!(out, "        let v = &self.{field}; {write}").unwrap();
        }
    }
    writeln!(out, "        out.push('}}');\n    }}\n}}").unwrap();
}

fn gen_enum_json(out: &mut String, ty: &str, ctors: &[&Def]) {
    writeln!(out, "impl crate::raw::ToJson for {} {{", pascal(split_ns(ty).1)).unwrap();
    writeln!(out, "    fn to_json(&self, out: &mut ::std::string::String) {{\n        match self {{").unwrap();
    for c in ctors {
        let v = pascal(split_ns(&c.name).1);
        if c.fields.is_empty() {
            writeln!(out, "            Self::{v} => out.push_str(r#\"{{\"_\":\"{}\"}}\"#),", c.name).unwrap();
        } else {
            let x = if is_inline(c) { "x" } else { "&**x" };
            writeln!(out, "            Self::{v}(x) => crate::raw::ToJson::to_json({x}, out),").unwrap();
        }
    }
    writeln!(out, "        }}\n    }}\n}}").unwrap();
}

fn gen_deserialize_bare(out: &mut String, d: &Def) {
    let name = pascal(split_ns(&d.name).1);
    writeln!(out, "impl {name} {{").unwrap();
    writeln!(out, "    #[doc(hidden)]").unwrap();
    writeln!(out, "    pub fn deserialize_bare(buf: &mut &[u8]) -> ::std::result::Result<Self, crate::raw::Error> {{")
        .unwrap();
    let names = gen_reads(out, d);
    writeln!(out, "        Ok(Self {{ {} }})\n    }}\n}}", names.join(", ")).unwrap();
}

/// `let` bindings that read `d`'s fields from `buf`, all but a generic query; returns their names.
fn gen_reads(out: &mut String, d: &Def) -> Vec<String> {
    let mut names = Vec::new();
    for f in d.fields.iter().filter(|f| !(d.generic && f.ty == "X")) {
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
    names
}

/// `json_of(body)`: any object or call as JSON, picked by its constructor id, and `tl_name(id)`.
fn gen_json_of(out: &mut String, defs: &[Def]) {
    writeln!(out, "pub(crate) fn tl_name(id: u32) -> ::std::option::Option<&'static str> {{").unwrap();
    writeln!(out, "    Some(match id {{").unwrap();
    for d in defs {
        writeln!(out, "        {:#010x} => {:?},", d.id, d.name).unwrap();
    }
    writeln!(out, "        _ => return None,\n    }})\n}}").unwrap();

    // Each call with fields reads them in its own function, so `?` stops at the call.
    let call_fn = |d: &Def| format!("__json_call_{:08x}", d.id);
    for d in defs.iter().filter(|d| d.function && !d.fields.is_empty()) {
        writeln!(
            out,
            "fn {}(buf: &mut &[u8]) -> ::std::result::Result<::std::string::String, crate::raw::Error> {{",
            call_fn(d)
        )
        .unwrap();
        let names = gen_reads(out, d);
        if !d.generic {
            let call = format!("{} {{ {} }}", path("functions", &d.name), names.join(", "));
            writeln!(out, "        Ok(crate::raw::ToJson::json(&{call}))").unwrap();
        } else {
            assert!(d.fields.last().is_some_and(|f| f.ty == "X"), "{}: the query must come last", d.name);
            writeln!(out, "        let mut out = ::std::string::String::from(r#\"{{\"_\":\"{}\"\"#);", d.name).unwrap();
            for f in d.fields.iter().filter(|f| f.ty != "#") {
                let key = format!("r#\",\"{}\":\"#", f.name);
                if f.ty == "X" {
                    let query = "json_of(buf).ok_or(crate::raw::Error::UnexpectedEof)?";
                    writeln!(out, "        out.push_str({key}); out.push_str(&{query});").unwrap();
                    continue;
                }
                let write = format!("{{ out.push_str({key}); crate::raw::ToJson::to_json(v, &mut out); }}");
                let field = ident(&f.name);
                if f.flag.is_some() && f.ty != "true" {
                    writeln!(out, "        if let Some(v) = &{field} {write}").unwrap();
                } else {
                    writeln!(out, "        let v = &{field}; {write}").unwrap();
                }
            }
            writeln!(out, "        out.push('}}');\n        Ok(out)").unwrap();
        }
        writeln!(out, "}}").unwrap();
    }

    writeln!(out, "pub(crate) fn json_of(body: &[u8]) -> ::std::option::Option<::std::string::String> {{").unwrap();
    writeln!(out, "    let id = u32::from_le_bytes(body.get(..4)?.try_into().ok()?);").unwrap();
    writeln!(out, "    let buf = &mut &body[4..];").unwrap();
    writeln!(out, "    match id {{").unwrap();
    let mut by_type: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for d in defs.iter().filter(|d| !d.function) {
        by_type.entry(&d.ty).or_default().push(d.id);
    }
    for (ty, ids) in &by_type {
        let ids: Vec<String> = ids.iter().map(|id| format!("{id:#010x}")).collect();
        let read = format!("<{} as crate::raw::Deserializable>::from_bytes(body)", path("base", ty));
        writeln!(out, "        {} => {read}.ok().map(|v| crate::raw::ToJson::json(&v)),", ids.join(" | ")).unwrap();
    }
    for d in defs.iter().filter(|d| d.function) {
        if d.fields.is_empty() {
            let call = format!("{} {{}}", path("functions", &d.name));
            writeln!(out, "        {:#010x} => Some(crate::raw::ToJson::json(&{call})),", d.id).unwrap();
        } else {
            writeln!(out, "        {:#010x} => {}(buf).ok(),", d.id, call_fn(d)).unwrap();
        }
    }
    writeln!(out, "        _ => None,\n    }}\n}}").unwrap();
}

/// Constructors of plain numbers that fit in 8 bytes are stored inline: they're no bigger than the
/// `Box` they replace. A base type never qualifies, so no enum can contain itself.
fn is_inline(d: &Def) -> bool {
    let (mut size, mut align) = (0, 1);
    for f in d.fields.iter().filter(|f| f.ty != "#") {
        let n = match f.ty.as_str() {
            "true" | "Bool" => 1,
            "int" => 4,
            "long" | "double" => 8,
            _ => return false,
        };
        // An `Option<bool>` hides its tag in the bool; other options pad the tag out to the alignment.
        size += if f.flag.is_some() && n > 1 { 2 * n } else { n };
        align = align.max(n);
    }
    usize::next_multiple_of(size, align) <= 8
}

fn gen_enum(out: &mut String, ty: &str, ctors: &[&Def], recursive: bool) {
    let name = pascal(split_ns(ty).1);
    let variants: Vec<String> = ctors.iter().map(|c| pascal(split_ns(&c.name).1)).collect();
    let unique: HashSet<_> = variants.iter().collect();
    assert_eq!(unique.len(), variants.len(), "two constructors of {ty} share a name");

    writeln!(out, "#[derive(Debug, Clone, PartialEq)]\npub enum {name} {{").unwrap();
    for (c, v) in ctors.iter().zip(&variants) {
        if c.fields.is_empty() {
            writeln!(out, "    {v},").unwrap();
        } else if is_inline(c) {
            writeln!(out, "    {v}({}),", path("types", &c.name)).unwrap();
        } else {
            writeln!(out, "    {v}(::std::boxed::Box<{}>),", path("types", &c.name)).unwrap();
        }
    }
    writeln!(out, "}}").unwrap();
    // `is_inline` guesses Rust's layout; this makes rustc check that no enum grew past a tag plus a box.
    writeln!(out, "const _: () = assert!(::std::mem::size_of::<{name}>() <= 16);").unwrap();

    writeln!(out, "impl crate::raw::Serializable for {name} {{").unwrap();
    writeln!(out, "    fn serialize(&self, buf: &mut ::std::vec::Vec<u8>) {{\n        match self {{").unwrap();
    for (c, v) in ctors.iter().zip(&variants) {
        if c.fields.is_empty() {
            writeln!(out, "            Self::{v} => crate::raw::Serializable::serialize(&{:#010x}u32, buf),", c.id)
                .unwrap();
        } else {
            let x = if is_inline(c) { "x" } else { "&**x" };
            writeln!(out, "            Self::{v}(x) => crate::raw::Serializable::serialize({x}, buf),").unwrap();
        }
    }
    writeln!(out, "        }}\n    }}\n}}").unwrap();

    writeln!(out, "impl crate::raw::Deserializable for {name} {{").unwrap();
    writeln!(out, "    fn deserialize(buf: &mut &[u8]) -> ::std::result::Result<Self, crate::raw::Error> {{").unwrap();
    let (open, close) = if recursive { ("crate::raw::core::nested(|| ", ")") } else { ("", "") };
    writeln!(out, "        {open}{{\n        let id = <u32 as crate::raw::Deserializable>::deserialize(buf)?;")
        .unwrap();
    writeln!(out, "        Ok(match id {{").unwrap();
    for (c, v) in ctors.iter().zip(&variants) {
        if c.fields.is_empty() {
            writeln!(out, "            {:#010x} => Self::{v},", c.id).unwrap();
        } else {
            let read = format!("{}::deserialize_bare(buf)?", path("types", &c.name));
            let read = if is_inline(c) { read } else { format!("::std::boxed::Box::new({read})") };
            writeln!(out, "            {:#010x} => Self::{v}({read}),", c.id).unwrap();
        }
    }
    writeln!(out, "            _ => return Err(crate::raw::Error::UnexpectedConstructor {{ id }}),").unwrap();
    writeln!(out, "        }})\n        }}{close}\n    }}\n}}").unwrap();
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

// Types that can contain themselves (RichText, PageBlock, JSONValue, ...): hostile data could nest them deep
// enough to overflow the stack, so their decoding goes through a depth check.
fn recursive_types(defs: &[Def]) -> HashSet<&str> {
    let mut edges: HashMap<&str, HashSet<&str>> = HashMap::new();
    for d in defs.iter().filter(|d| !d.function) {
        let children = edges.entry(&d.ty).or_default();
        for f in &d.fields {
            let mut t = f.ty.as_str();
            while let Some(inner) = vector_inner(t) {
                t = inner;
            }
            children.insert(t);
        }
    }
    let reaches_itself = |ty: &str| {
        let (mut seen, mut stack) = (HashSet::new(), vec![ty]);
        while let Some(t) = stack.pop() {
            for &c in edges.get(t).into_iter().flatten() {
                if c == ty {
                    return true;
                }
                if seen.insert(c) {
                    stack.push(c);
                }
            }
        }
        false
    };
    edges.keys().copied().filter(|ty| reaches_itself(ty)).collect()
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
    let (head, ret) = if d.generic {
        (format!("impl<X: crate::raw::RemoteCall> crate::raw::RemoteCall for {name}<X>"), "X::Return".into())
    } else {
        (format!("impl crate::raw::RemoteCall for {name}"), rust_ty(&d.ty, false))
    };
    writeln!(out, "{head} {{ const NAME: &'static str = {:?}; type Return = {ret}; }}", d.name).unwrap();
}

/// `json`: also implement `ToJson` for every type and function (the `json` cargo feature).
pub fn generate(api: &str, mtproto: &str, json: bool) -> String {
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
            let out = mods.entry(("functions", ns)).or_default();
            gen_function(out, d);
            if json {
                gen_struct_json(out, d);
            }
        } else {
            by_type.entry(&d.ty).or_default().push(d);
            if !d.fields.is_empty() {
                let out = mods.entry(("types", ns)).or_default();
                gen_struct(out, d);
                gen_deserialize_bare(out, d);
                if json {
                    gen_struct_json(out, d);
                }
            }
        }
    }
    let recursive = recursive_types(&defs);
    for (ty, ctors) in &by_type {
        let out = mods.entry(("base", split_ns(ty).0)).or_default();
        gen_enum(out, ty, ctors, recursive.contains(ty));
        if json {
            gen_enum_json(out, ty, ctors);
        }
        if *ty == "Update" {
            gen_update_counters(out, ctors);
        }
    }

    let mut credentials: Vec<u32> = defs.iter().filter(|d| credential(d)).map(|d| d.id).collect();
    credentials.sort_unstable();
    let mut out = format!("/// The API layer this code was generated from.\npub const LAYER: i32 = {layer};\n");
    writeln!(out, "pub(crate) const CREDENTIALS: [u32; {}] = {credentials:#010x?};", credentials.len()).unwrap();
    if json {
        gen_json_of(&mut out, &defs);
    }
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
