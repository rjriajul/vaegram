# Reads spec.json, vendored from github.com/PaulSonOfLars/telegram-bot-api-spec (MIT license).
"""Write src/bot_api/raw/types.rs and src/bot_api/raw/methods.rs from tools/botapi/spec.json.

usage: python tools/botapi/gen.py
"""
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = json.loads((ROOT / "tools/botapi/spec.json").read_text("utf-8"))
OUT = ROOT / "src/bot_api/raw"
WIDTH = 120
# Size difference (estimated bytes) between the two largest enum variants above which the largest gets boxed.
BOX_MARGIN = 100
# Estimated bytes above which a struct held in another struct's field is boxed.
LARGE_FIELD = 512

SCALARS = {"Integer": "i64", "Float": "f64", "Boolean": "bool", "True": "bool", "String": "String"}
SIZES = {"i64": 8, "f64": 8, "bool": 1, "String": 24, "InputFile": 48, "ChatId": 32, "serde_json::Value": 80}
NICHE = {"String", "bool", "InputFile", "ChatId", "serde_json::Value"}
KEYWORDS = {
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "static",
    "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do",
    "final", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
}
# Field and return unions no abstract spec type covers, named by hand.
UNIONS = {
    ("ForceReply", "InlineKeyboardMarkup", "ReplyKeyboardMarkup", "ReplyKeyboardRemove"):
        ("ReplyMarkup", "A keyboard to attach to a message, or an instruction to remove or replace one."),
    ("InputMediaAnimation", "InputMediaAudio", "InputMediaDocument", "InputMediaPhoto", "InputMediaVideo",
     "InputMediaVoiceNote"):
        ("InputRichMedia", "A media element embedded in an outgoing rich message."),
    ("Boolean", "Message"): ("MessageOrBool", "The edited message, or `true` when it was an inline message."),
}
UNION_VARIANTS = {"Boolean": "Bool", "True": "Bool", "String": "Text", "Integer": "Int", "Float": "Float"}
# Variants share their fields, so serde can't pick one by shape; src/bot_api/raw/mod.rs implements Deserialize.
CUSTOM_DE = {"MaybeInaccessibleMessage"}
TAG_RE = re.compile(r"(?:always|must be) [“\"]?([a-z0-9_]+)[”\"]?")


def fail(msg):
    sys.exit(f"gen.py: {msg}")


# ---------- docs ----------

def first_sentence(text):
    text = " ".join(text.split())
    for m in re.finditer(r"[.!?](?=\s+[A-Z“\"(]|$)", text):
        head = text[: m.end()]
        if not re.search(r"\b(?:e\.g|i\.e|etc|vs|approx)\.$", head):
            return head
    return text


def escape(text):
    text = text.replace("\\", "\\\\").replace("[", "\\[").replace("]", "\\]").replace("<", "\\<").replace(">", "\\>")
    return re.sub(r"https?://[^\s,;)]*[^\s,;).]", lambda m: f"<{m[0]}>", text)


MARKER = re.compile(r"^(?:[-+*#>]|\d+[.)])$")


def doc(text, indent):
    prefix = " " * indent + "/// "
    words, lines, line = escape(text).split(), [], []
    for w in words:
        if line and len(prefix) + len(" ".join(line + [w])) > WIDTH:
            lines.append(line)
            line = []
            # A line starting with a list or heading marker would turn the doc into markdown structure.
            if MARKER.match(w) and len(lines[-1]) > 1:
                line.append(lines[-1].pop())
        line.append(w)
    lines.append(line)
    return [prefix + " ".join(l) for l in lines]


def type_doc(t):
    return first_sentence((t.get("description") or [""])[0]) or f"The `{t['name']}` object."


def field_doc(f):
    text = re.sub(r"^Optional\.\s*", "", f["description"].strip())
    return first_sentence(text) or f"The `{f['name']}` field."


# ---------- model ----------

TYPES, METHODS = SPEC["types"], SPEC["methods"]
STRUCTS = [n for n, t in TYPES.items() if not t.get("subtypes") and n != "InputFile"]
ABSTRACT = {n: t["subtypes"] for n, t in TYPES.items() if t.get("subtypes")}


def camel_words(name):
    return re.findall(r"[A-Z][a-z0-9]*", name)


def variant_names(members):
    """Variant names for `members`, dropping a camel-case prefix and suffix every member shares."""
    names = [UNION_VARIANTS.get(m, m) if not m.startswith("Array of ") else "Array" for m in members]
    words = [camel_words(n) for n in names]
    if len(words) > 1 and all("".join(w) == n for w, n in zip(words, names)):
        pre = 0
        while all(len(w) > pre + 1 for w in words) and len({w[pre] for w in words}) == 1:
            pre += 1
        words = [w[pre:] for w in words]
        suf = 0
        while all(len(w) > suf + 1 for w in words) and len({w[-1 - suf] for w in words}) == 1:
            suf += 1
        words = [w[: len(w) - suf] for w in words]
        names = ["".join(w) for w in words]
    if len(set(names)) != len(names) or "Unknown" in names:
        fail(f"variant names clash for {members}")
    return names


def discriminator(name):
    for f in TYPES[name].get("fields", []):
        m = TAG_RE.search(f["description"])
        if m and f["types"] == ["String"]:
            return f["name"], m[1]
    return None


class Enum:
    def __init__(self, name, doc, members, unknown, kind=None):
        self.name, self.doc, self.members, self.unknown = name, doc, members, unknown
        typed = [m for m in members if m in TYPES]
        tags = [discriminator(m) for m in typed]
        # Tagged needs one tag field on every member and a distinct value each; InlineQueryResult repeats "audio".
        unique = all(tags) and len({t[0] for t in tags}) == 1 and len({t[1] for t in tags}) == len(tags)
        if kind:
            self.kind = kind
        else:
            self.kind = "tagged" if typed == members and unique else "untagged"
        self.tag = tags[0][0] if self.kind == "tagged" else None
        self.values = {m: t[1] for m, t in zip(typed, tags)} if self.kind == "tagged" else {}
        self.variants = dict(zip(members, variant_names(members)))


ENUMS = {}
for name, subs in ABSTRACT.items():
    typed = [s for s in subs if s in TYPES]
    doc_ = type_doc(TYPES[name])
    if typed != subs:
        # Plain values and object kinds mixed, e.g. RichText: an untagged enum over the plain ones and a tagged one.
        obj = f"{name}Object"
        ENUMS[obj] = Enum(obj, f"The object kinds of `{name}`.", typed, True)
        outer = [s for s in subs if s not in TYPES] + [obj]
        ENUMS[name] = Enum(name, doc_, outer, True, "untagged")
        ENUMS[name].variants[obj] = "Object"
    else:
        ENUMS[name] = Enum(name, doc_, subs, True, "custom" if name in CUSTOM_DE else None)


def base(t):
    return re.sub(r"^(Array of )+", "", t)


def find_union(members):
    """The enum covering the spec types `members`: the smallest abstract type, else a hand-named union."""
    if all(m in TYPES for m in members):
        covering = [n for n, e in ENUMS.items() if n in ABSTRACT and set(members) <= set(e.members)]
        if covering:
            return min(covering, key=lambda n: len(ENUMS[n].members))
    key = tuple(sorted(members))
    if key not in UNIONS:
        fail(f"no name for the union {members}; add it to UNIONS")
    name, doc_ = UNIONS[key]
    if name not in ENUMS:
        ENUMS[name] = Enum(name, doc_, list(members), False)
    return name


def union_of(types):
    s = set(types)
    if s == {"Integer", "String"}:
        return ("named", "ChatId")
    if s == {"InputFile", "String"}:
        return ("named", "InputFile")
    if all(t.startswith("Array of ") for t in types):
        return ("vec", union_of([t[len("Array of "):] for t in types]))
    return ("named", find_union(types))


# Register every union first: they decide which structs lose their tag field.
for kind in (TYPES, METHODS):
    for t in kind.values():
        for f in t.get("fields", []):
            if len(f["types"]) > 1:
                union_of(f["types"])
for m in METHODS.values():
    if len(m["returns"]) > 1:
        union_of(m["returns"])

PARENTS = {}
for e in ENUMS.values():
    for m in e.members:
        PARENTS.setdefault(m, []).append(e)
OMIT = {}
for s, parents in PARENTS.items():
    tags = {p.tag for p in parents}
    if any(p.kind == "tagged" for p in parents):
        if len(tags) != 1:
            fail(f"{s} is in tagged and untagged enums, or tagged on different fields: {[p.name for p in parents]}")
        OMIT[s] = tags.pop()


def one(t):
    if t.startswith("Array of "):
        return ("vec", one(t[len("Array of "):]))
    if t in SCALARS:
        return ("prim", SCALARS[t])
    if t == "InputFile":
        return ("named", "InputFile")
    if t not in TYPES:
        fail(f"unknown type {t}")
    if t in OMIT:
        # Alone, a struct that lost its tag field would serialize without it; its enum adds the tag back.
        tagged = [p for p in PARENTS[t] if p.kind == "tagged"]
        return ("named", min(tagged, key=lambda p: (p.name not in ABSTRACT, len(p.members))).name)
    return ("named", t)


def resolve(types):
    return one(types[0]) if len(types) == 1 else union_of(types)


def member_ir(m):
    return one(m) if m in SCALARS or m.startswith("Array of ") else ("named", m)


class Field:
    def __init__(self, f):
        self.name, self.required, self.doc = f["name"], f["required"], field_doc(f)
        self.rust = f"r#{self.name}" if self.name in KEYWORDS else self.name
        # Media objects type their files as String but accept attach://, which InputFile::Upload produces.
        attach = f["types"] == ["String"] and "attach://" in f["description"]
        self.ir = ("named", "InputFile") if attach else resolve(f["types"])
        self.boxed = False


STRUCT_FIELDS = {
    n: [Field(f) for f in TYPES[n].get("fields", []) if f["name"] != OMIT.get(n)] for n in STRUCTS
}
VARIANT_IR = {e.name: {m: member_ir(m) for m in e.members} for e in ENUMS.values()}
VARIANT_BOX = {e.name: set() for e in ENUMS.values()}

# ---------- recursion ----------

def edges():
    """Edges that make a type's size depend on another's: by value, not behind a Vec or a Box."""
    out = {}
    for n, fields in STRUCT_FIELDS.items():
        out[n] = [(f.ir[1], ("field", n, f)) for f in fields if f.ir[0] == "named" and not f.boxed]
    for e in ENUMS.values():
        out[e.name] = [(ir[1], ("variant", e.name, m)) for m, ir in VARIANT_IR[e.name].items()
                       if ir[0] == "named" and m not in VARIANT_BOX[e.name]]
    return out


def sccs(graph):
    index, low, stack, on, result, counter = {}, {}, [], set(), [], [0]
    sys.setrecursionlimit(10000)

    def visit(v):
        index[v] = low[v] = counter[0]
        counter[0] += 1
        stack.append(v)
        on.add(v)
        for w, _ in graph.get(v, []):
            if w not in graph:
                continue
            if w not in index:
                visit(w)
                low[v] = min(low[v], low[w])
            elif w in on:
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            comp = set()
            while True:
                w = stack.pop()
                on.remove(w)
                comp.add(w)
                if w == v:
                    break
            result.append(comp)

    for v in graph:
        if v not in index:
            visit(v)
    comp_of = {}
    for c in result:
        for v in c:
            comp_of[v] = id(c)
    return comp_of


def cyclic_edges(graph):
    comp = sccs(graph)
    return [(v, w, tag) for v, es in graph.items() for w, tag in es if w in comp and comp[v] == comp[w]]


def box(tag):
    if tag[0] == "field":
        tag[2].boxed = True
    else:
        VARIANT_BOX[tag[1]].add(tag[2])


CYCLE_BOXES = []
ORDER = {n: i for i, n in enumerate(list(TYPES) + list(ENUMS))}
while True:
    cyc = cyclic_edges(edges())
    if not cyc:
        break
    out_degree = {}
    for v, _, _ in cyc:
        out_degree[v] = out_degree.get(v, 0) + 1
    variants = [e for e in cyc if e[2][0] == "variant"]
    if variants:
        # A variant of an enum with few in-cycle variants breaks the most cycles per Box, e.g. RichText::Object.
        v, w, tag = min(variants, key=lambda e: (out_degree[e[0]], ORDER.get(e[0], 1 << 30), e[2][2]))
    else:
        # Box the in-cycle field that leads back to the type defined earliest, closing the cycle where it starts.
        v, w, tag = min(cyc, key=lambda e: (ORDER.get(e[1], 1 << 30), ORDER.get(e[0], 1 << 30), e[2][2].name))
    box(tag)
    CYCLE_BOXES.append(tag)

# ---------- sizes: stack use and clippy::large_enum_variant ----------

SIZE_MEMO = {}


def ir_size(ir, boxed=False):
    if boxed:
        return 8
    kind, inner = ir
    if kind == "vec":
        return 24
    if kind == "prim":
        return SIZES[inner]
    return type_size(inner)


def type_size(name):
    if name in SIZES:
        return SIZES[name]
    if name not in SIZE_MEMO:
        if name in STRUCT_FIELDS:
            total = 0
            for f in STRUCT_FIELDS[name]:
                # Update holds a dozen optional Messages; unboxed they'd take hundreds of KB of stack to deserialize.
                if f.ir[0] == "named" and f.ir[1] in STRUCT_FIELDS and not f.boxed and type_size(f.ir[1]) > LARGE_FIELD:
                    f.boxed = True
                    FIELD_BOXES.append((name, f.name))
                s = ir_size(f.ir, f.boxed)
                plain = f.boxed or f.ir[0] == "vec" or (f.ir[0] == "prim" and f.ir[1] in NICHE)
                total += s if f.required or plain else s + 8
            SIZE_MEMO[name] = (total + 7) // 8 * 8
        else:
            SIZE_MEMO[name] = max(variant_sizes(ENUMS[name]).values(), default=0) + 8
    return SIZE_MEMO[name]


def variant_sizes(e):
    sizes = {m: ir_size(ir, m in VARIANT_BOX[e.name]) for m, ir in VARIANT_IR[e.name].items()}
    if e.unknown:
        sizes[None] = 0 if e.kind == "tagged" else SIZES["serde_json::Value"]
    return sizes


FIELD_BOXES = []
for name in STRUCTS:
    type_size(name)
SIZE_MEMO.clear()
SIZE_BOXES = []
for e in ENUMS.values():
    while True:
        sizes = sorted(variant_sizes(e).items(), key=lambda kv: -kv[1])
        if len(sizes) < 2 or sizes[0][0] is None or sizes[0][1] - sizes[1][1] <= BOX_MARGIN:
            break
        VARIANT_BOX[e.name].add(sizes[0][0])
        SIZE_BOXES.append((e.name, sizes[0][0]))
        SIZE_MEMO.clear()

# ---------- rendering ----------

USED = set()


def render(ir, boxed=False):
    kind, inner = ir
    if kind == "vec":
        s = f"Vec<{render(inner)}>"
    elif kind == "prim":
        s = inner
    else:
        USED.add(inner)
        s = inner
    return f"Box<{s}>" if boxed else s


def render_fields(fields, out):
    for f in fields:
        ty = render(f.ir, f.boxed)
        out += doc(f.doc, 4)
        if not f.required:
            out.append('    #[serde(default, skip_serializing_if = "Option::is_none")]')
            ty = f"Option<{ty}>"
        out.append(f"    pub {f.rust}: {ty},")


def render_struct(name, docline, fields, derives):
    out = doc(docline, 0) + [f"#[derive({derives})]"]
    if not fields:
        return out + [f"pub struct {name} {{}}"]
    out.append(f"pub struct {name} {{")
    render_fields(fields, out)
    return out + ["}"]


def render_enum(e):
    derive = "Debug, Clone, PartialEq, Serialize" + ("" if e.kind == "custom" else ", Deserialize")
    out = doc(e.doc, 0) + [f"#[derive({derive})]"]
    if e.kind == "tagged":
        out.append(f'#[serde(tag = "{e.tag}")]')
    else:
        out.append("#[serde(untagged)]")
    out.append(f"pub enum {e.name} {{")
    members = e.members
    if e.kind == "untagged" and e.unknown:
        # serde tries untagged variants in order, so those needing the most fields go first.
        members = sorted(members, key=lambda m: -sum(f.required for f in STRUCT_FIELDS.get(m, [])))
    for m in members:
        ir = VARIANT_IR[e.name][m]
        if m in TYPES:
            text = type_doc(TYPES[m])
        elif m in ENUMS:
            text = ENUMS[m].doc
        else:
            text = {"String": "Plain text.", "Boolean": "`true`.", "True": "`true`."}.get(m, f"A list of `{base(m)}`.")
        out += doc(text, 4)
        if e.kind == "tagged":
            out.append(f'    #[serde(rename = "{e.values[m]}")]')
        out.append(f"    {e.variants[m]}({render(ir, m in VARIANT_BOX[e.name])}),")
    if e.unknown:
        out.append("    /// A kind added to the Bot API after this layer was generated.")
        if e.kind == "tagged":
            out += ["    #[serde(other)]", "    Unknown,"]
        else:
            out.append("    Unknown(serde_json::Value),")
    return out + ["}"]


def header(what):
    return [
        f"// Generated by tools/botapi/gen.py from tools/botapi/spec.json ({SPEC['version']}); do not edit.",
        f"//! {what}",
        "",
    ]


def imports(local):
    crate = sorted({"InputFile"} & USED | ({"Request"} if local == "methods" else set()))
    lines = []
    if crate:
        names = crate[0] if len(crate) == 1 else "{" + ", ".join(crate) + "}"
        lines += [f"use crate::bot_api::{names};", ""]
    lines += ["use serde::{Deserialize, Serialize};" if local == "types" else "use serde::Serialize;", ""]
    if "ChatId" in USED:
        lines += ["use super::ChatId;", ""]
    if local == "methods":
        lines += ["use super::types::*;", ""]
    return lines


def write(path, lines):
    text = "\n".join(lines).rstrip("\n") + "\n"
    long = [l for l in text.splitlines() if len(l) > WIDTH]
    if long:
        fail(f"lines over {WIDTH} chars in {path.name}: {long[:3]}")
    path.write_text(text, "utf-8", newline="\n")


# types.rs: structs in spec order, each abstract type's enum in its place, unions after.
body, done = [], set()
type_derive = "Debug, Clone, PartialEq, Serialize, Deserialize"
for name, t in TYPES.items():
    if name == "InputFile":
        continue
    if name in ENUMS:
        # An abstract type's helper enum (RichTextObject) follows it.
        for e in [ENUMS[name]] + [ENUMS[m] for m in ENUMS[name].members if m in ENUMS and m not in TYPES]:
            body += [""] + render_enum(e)
            done.add(e.name)
    else:
        body += [""] + render_struct(name, type_doc(t), STRUCT_FIELDS[name], type_derive)
for name, e in ENUMS.items():
    if name not in TYPES and name not in done:
        body += [""] + render_enum(e)
write(OUT / "types.rs", header("The Bot API's types.") + imports("types") + body[1:])

USED.clear()
mbody = []
for name, m in METHODS.items():
    fields = [Field(f) for f in m.get("fields", [])]
    rust = name[0].upper() + name[1:]
    defaultable = all(not f.required or f.ir[0] == "vec" or f.ir[0] == "prim" for f in fields)
    derive = "Debug, Clone, PartialEq, " + ("Default, " if defaultable else "") + "Serialize"
    response = render(resolve(m["returns"]))
    mbody += [""] + render_struct(rust, first_sentence(m["description"][0]), fields, derive)
    mbody += ["", f"impl Request for {rust} {{", f'    const NAME: &\'static str = "{name}";',
              f"    type Response = {response};", "}"]
write(OUT / "methods.rs", header("The Bot API's methods, one struct each.") + imports("methods") + mbody[1:])

print(f"{SPEC['version']}: {len(STRUCTS)} structs, {len(ENUMS)} enums, {len(METHODS)} methods")
for e in ENUMS.values():
    how = f"tag = {e.tag}" if e.kind == "tagged" else e.kind
    print(f"  enum {e.name}: {how}, {len(e.members)} variants" + ("" if e.unknown else " (union)"))
print("  tag fields dropped:", len(OMIT))
for tag in CYCLE_BOXES:
    print(f"  box (cycle): {tag[1]}.{tag[2].name if tag[0] == 'field' else ENUMS[tag[1]].variants[tag[2]]}")
for e, m in SIZE_BOXES:
    print(f"  box (size): {e}.{ENUMS[e].variants[m]}")
for s, f in FIELD_BOXES:
    print(f"  box (large field): {s}.{f}")
