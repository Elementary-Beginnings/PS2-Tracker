//! Reader for the binary format of SharpSerializer ("burst" mode, MIT licensed
//! library; format taken from its public source: `Elements.cs`,
//! `BinaryReaderTools.cs`, `BinaryPropertyDeserializer.cs`).
//!
//! Layout of a property: element id (byte), name (bool + .NET string), type name
//! (bool + .NET string), then per element: reference id (number) for `*WithId` and
//! `Reference`; properties (count + properties) for objects; element type,
//! properties and items for collections; key type, value type, properties and
//! pairs for dictionaries; element type, lower bound and items for arrays; value
//! (bool + data) for simple values. Numbers are size byte (0/1/2/4) + value.
//!
//! The writer omits the type of a simple value when it equals the declared
//! .NET property type; that type is then not in the file. It is inferred from the
//! member name, else the candidates are tried and the one after which the next
//! property header is plausible wins. Never instantiates anything.

use std::collections::HashMap;

use crate::pakdoc::{Doc, Node};

const COLLECTION: u8 = 1;
const COMPLEX: u8 = 2;
const DICTIONARY: u8 = 3;
const MULTI_ARRAY: u8 = 4;
const NULL: u8 = 5;
const SIMPLE: u8 = 6;
const SINGLE_ARRAY: u8 = 7;
const COMPLEX_ID: u8 = 8;
const REFERENCE: u8 = 9;
const COLLECTION_ID: u8 = 10;
const DICTIONARY_ID: u8 = 11;
const SINGLE_ARRAY_ID: u8 = 12;
const MULTI_ARRAY_ID: u8 = 13;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Ty {
    Str,
    Bool,
    Byte,
    SByte,
    Char,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    Decimal,
    Guid,
    Bytes,
}

/// Tried in this order when neither the file nor the member name gives the
/// type; the first one after which the next property header is plausible wins.
const CANDIDATES: [Ty; 11] = [
    Ty::Str,
    Ty::Bool,
    Ty::I32,
    Ty::Bytes,
    Ty::I64,
    Ty::F64,
    Ty::F32,
    Ty::Guid,
    Ty::Decimal,
    Ty::Byte,
    Ty::I16,
];

/// Starts like a SharpSerializer stream (element id, name present, name)?
pub fn looks_like_sharp(head: &[u8]) -> bool {
    head.len() > 3 && (1..=13).contains(&head[0]) && head[1] == 1 && head[2] > 0 && head[2] < 0x80
}

#[derive(Debug)]
pub struct SharpError {
    pub offset: usize,
    pub reason: String,
}

impl std::fmt::Display for SharpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "corrupt PAK record at offset {}: {}",
            self.offset, self.reason
        )
    }
}

struct P<'a> {
    d: &'a [u8],
    pos: usize,
    nodes: usize,
    max_bytes: usize,
    targets: HashMap<i64, Node>,
}

type R<T> = Result<T, SharpError>;

pub fn parse(d: &[u8], max_array_bytes: usize) -> R<Doc> {
    let mut p = P {
        d,
        pos: 0,
        nodes: 0,
        max_bytes: max_array_bytes,
        targets: HashMap::new(),
    };
    let id = p.u8()?;
    let (_, node) = p.property(id, None, 0)?;
    Ok(Doc {
        root: node,
        targets: p.targets,
    })
}

impl<'a> P<'a> {
    fn err(&self, r: impl Into<String>) -> SharpError {
        SharpError {
            offset: self.pos,
            reason: r.into(),
        }
    }
    fn take(&mut self, n: usize) -> R<&'a [u8]> {
        if n > self.d.len() - self.pos {
            return Err(self.err(format!("needs {n} bytes, {} left", self.d.len() - self.pos)));
        }
        let s = &self.d[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> R<u8> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> R<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            b => Err(self.err(format!("invalid boolean {b}"))),
        }
    }
    fn dotnet_string(&mut self) -> R<String> {
        let mut len = 0usize;
        for i in 0..5 {
            let b = self.u8()?;
            len |= usize::from(b & 0x7f) << (7 * i);
            if b & 0x80 == 0 {
                break;
            }
            if i == 4 {
                return Err(self.err("string length too long"));
            }
        }
        let b = self.take(len)?;
        String::from_utf8(b.to_vec()).map_err(|_| self.err("string is not UTF-8"))
    }
    fn opt_string(&mut self) -> R<Option<String>> {
        Ok(if self.bool()? {
            Some(self.dotnet_string()?)
        } else {
            None
        })
    }
    fn number(&mut self) -> R<i64> {
        Ok(match self.u8()? {
            0 => 0,
            1 => i64::from(self.u8()?),
            2 => i64::from(i16::from_le_bytes(self.take(2)?.try_into().expect("2"))),
            _ => i64::from(i32::from_le_bytes(self.take(4)?.try_into().expect("4"))),
        })
    }
    fn count(&mut self) -> R<usize> {
        let n = self.number()?;
        let n = usize::try_from(n).map_err(|_| self.err("negative count"))?;
        // every element needs at least one byte
        if n > self.d.len() - self.pos {
            return Err(self.err(format!("count {n} larger than the rest of the file")));
        }
        Ok(n)
    }

    fn value(&mut self, ty: Ty) -> R<Node> {
        if !self.bool()? {
            return Ok(Node::Null);
        }
        let le = |p: &mut Self, n: usize| -> R<[u8; 8]> {
            let mut b = [0u8; 8];
            b[..n].copy_from_slice(p.take(n)?);
            Ok(b)
        };
        Ok(match ty {
            Ty::Str => Node::Str(self.dotnet_string()?),
            Ty::Bool => Node::Bool(self.bool()?),
            Ty::Byte => Node::Int(i64::from(self.u8()?)),
            Ty::SByte => Node::Int(i64::from(self.u8()? as i8)),
            Ty::Char => {
                let first = self.u8()?;
                let n = match first {
                    0x00..=0x7f => 0,
                    0xc0..=0xdf => 1,
                    0xe0..=0xef => 2,
                    0xf0..=0xf7 => 3,
                    _ => return Err(self.err("invalid char")),
                };
                let mut v = vec![first];
                v.extend_from_slice(self.take(n)?);
                Node::Str(String::from_utf8(v).map_err(|_| self.err("invalid char"))?)
            }
            Ty::I16 => Node::Int(i64::from(i16::from_le_bytes(
                self.take(2)?.try_into().expect("2"),
            ))),
            Ty::U16 => Node::Int(i64::from(u16::from_le_bytes(
                self.take(2)?.try_into().expect("2"),
            ))),
            Ty::I32 => Node::Int(i64::from(i32::from_le_bytes(
                self.take(4)?.try_into().expect("4"),
            ))),
            Ty::U32 => Node::Int(i64::from(u32::from_le_bytes(
                self.take(4)?.try_into().expect("4"),
            ))),
            Ty::I64 => Node::Int(i64::from_le_bytes(le(self, 8)?)),
            Ty::U64 => {
                Node::Int(i64::try_from(u64::from_le_bytes(le(self, 8)?)).unwrap_or(i64::MAX))
            }
            Ty::F32 => Node::Float(f64::from(f32::from_le_bytes(
                self.take(4)?.try_into().expect("4"),
            ))),
            Ty::F64 => Node::Float(f64::from_le_bytes(le(self, 8)?)),
            Ty::Decimal => {
                self.take(16)?;
                Node::Float(0.0)
            }
            Ty::Guid => Node::Bytes(self.take(16)?.to_vec()),
            Ty::Bytes => {
                let n = self.number()?;
                let n = usize::try_from(n).map_err(|_| self.err("negative array length"))?;
                if n > self.max_bytes {
                    return Err(self.err(format!("byte array of {n} bytes over the limit")));
                }
                if n == 0 {
                    Node::Null
                } else {
                    Node::Bytes(self.take(n)?.to_vec())
                }
            }
        })
    }

    /// Next bytes look like a property header (element id, name flag, name)?
    fn header_plausible(&self, at: usize) -> bool {
        let d = self.d;
        if at == d.len() {
            return true;
        }
        let Some(&id) = d.get(at) else { return false };
        if !(1..=13).contains(&id) {
            return false;
        }
        match d.get(at + 1) {
            Some(0) => true,
            Some(1) => {
                let Some(&len) = d.get(at + 2) else {
                    return false;
                };
                let len = usize::from(len);
                len > 0
                    && len < 0x80
                    && d.get(at + 3..at + 3 + len).is_some_and(|n| {
                        std::str::from_utf8(n).is_ok_and(|s| {
                            s.chars()
                                .all(|c| c.is_alphanumeric() || "_<>`.[]+".contains(c))
                        })
                    })
            }
            _ => false,
        }
    }

    /// Simple value with an omitted type: name heuristics first, then the
    /// candidates; `last` = no further property follows in this object.
    fn untyped(&mut self, name: Option<&str>, last: bool, owner: &str) -> R<Node> {
        let first = guess(name.unwrap_or(""));
        let order: Vec<Ty> = first
            .into_iter()
            .chain(CANDIDATES.iter().copied().filter(|t| Some(*t) != first))
            .collect();
        let start = self.pos;
        for ty in order {
            self.pos = start;
            if let Ok(v) = self.value(ty) {
                let plausible = match &v {
                    Node::Str(s) => !s.chars().any(|c| c.is_control() && !"\r\n\t".contains(c)),
                    _ => true,
                };
                if plausible && (last || self.header_plausible(self.pos)) {
                    tracing::debug!(member = name.unwrap_or("(item)"), owner, ty = ?ty, offset = start, "untyped value");
                    return Ok(v);
                }
            }
        }
        self.pos = start;
        Err(self.err(format!(
            "value of {} in {} has an unknown type",
            name.unwrap_or("(item)"),
            if owner.is_empty() { "(object)" } else { owner }
        )))
    }

    fn properties(&mut self, depth: usize, owner: &str) -> R<Vec<(String, Node)>> {
        let n = self.count()?;
        let mut out = Vec::with_capacity(n.min(64));
        for i in 0..n {
            let id = self.u8()?;
            let (name, v) = self.property_full(id, None, depth + 1, i + 1 == n, owner)?;
            out.push((name.unwrap_or_default(), v));
        }
        Ok(out)
    }

    fn items(&mut self, elem: Option<Ty>, depth: usize) -> R<Vec<Node>> {
        let n = self.count()?;
        let mut out = Vec::with_capacity(n.min(4096));
        for i in 0..n {
            let id = self.u8()?;
            let (_, v) = self.property_with_last(id, elem, depth + 1, i + 1 == n)?;
            out.push(v);
        }
        Ok(out)
    }

    fn property(
        &mut self,
        id: u8,
        expected: Option<Ty>,
        depth: usize,
    ) -> R<(Option<String>, Node)> {
        self.property_with_last(id, expected, depth, true)
    }

    fn property_with_last(
        &mut self,
        id: u8,
        expected: Option<Ty>,
        depth: usize,
        last: bool,
    ) -> R<(Option<String>, Node)> {
        self.property_full(id, expected, depth, last, "")
    }

    fn property_full(
        &mut self,
        id: u8,
        expected: Option<Ty>,
        depth: usize,
        last: bool,
        owner: &str,
    ) -> R<(Option<String>, Node)> {
        if depth > 64 {
            return Err(self.err("nesting too deep"));
        }
        self.nodes += 1;
        if self.nodes > 2_000_000 {
            return Err(self.err("too many elements"));
        }
        let name = self.opt_string()?;
        let ty_name = self.opt_string()?;
        let mut ref_id = None;
        if id == REFERENCE
            || matches!(
                id,
                COMPLEX_ID | COLLECTION_ID | DICTIONARY_ID | SINGLE_ARRAY_ID | MULTI_ARRAY_ID
            )
        {
            let r = self.number()?;
            if id == REFERENCE {
                return Ok((name, Node::Ref(r)));
            }
            ref_id = Some(r);
        }
        let ty_label = ty_name.clone().unwrap_or_default();
        let node = match id {
            NULL => Node::Null,
            SIMPLE => match ty_name.as_deref().map(simple_type).or(expected.map(Some)) {
                Some(Some(t)) => self.value(t)?,
                Some(None) => {
                    // enum (non-system type): written as Int32
                    self.value(Ty::I32)?
                }
                None => self.untyped(name.as_deref(), last, owner)?,
            },
            COMPLEX | COMPLEX_ID => {
                let fields = self.properties(depth, &ty_label)?;
                Node::Obj {
                    ty: ty_label,
                    fields,
                }
            }
            COLLECTION | COLLECTION_ID => {
                let elem = self.opt_string()?;
                let props = self.properties(depth, &ty_label)?;
                let items = self.items(elem.as_deref().and_then(simple_type), depth)?;
                if props.is_empty() {
                    Node::List(items)
                } else {
                    let mut fields = props;
                    fields.push(("items".into(), Node::List(items)));
                    Node::Obj {
                        ty: ty_label,
                        fields,
                    }
                }
            }
            DICTIONARY | DICTIONARY_ID => {
                let kt = self.opt_string()?.as_deref().and_then(simple_type);
                let vt = self.opt_string()?.as_deref().and_then(simple_type);
                let _props = self.properties(depth, &ty_label)?;
                let n = self.count()?;
                let mut pairs = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    let kid = self.u8()?;
                    let (_, k) = self.property_with_last(kid, kt, depth + 1, false)?;
                    let vid = self.u8()?;
                    let (_, v) = self.property_with_last(vid, vt, depth + 1, true)?;
                    pairs.push((k, v));
                }
                Node::Map(pairs)
            }
            SINGLE_ARRAY | SINGLE_ARRAY_ID => {
                let elem = self.opt_string()?;
                let _lower = self.number()?;
                Node::List(self.items(elem.as_deref().and_then(simple_type), depth)?)
            }
            MULTI_ARRAY | MULTI_ARRAY_ID => {
                let elem = self.opt_string()?.as_deref().and_then(simple_type);
                let dims = self.count()?;
                for _ in 0..dims {
                    self.number()?;
                    self.number()?;
                }
                let n = self.count()?;
                let mut out = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    let k = self.count()?;
                    for _ in 0..k {
                        self.number()?;
                    }
                    let eid = self.u8()?;
                    out.push(self.property_with_last(eid, elem, depth + 1, true)?.1);
                }
                Node::List(out)
            }
            other => return Err(self.err(format!("unknown element {other}"))),
        };
        if let Some(r) = ref_id {
            self.targets.insert(r, node);
            return Ok((name, Node::Ref(r)));
        }
        Ok((name, node))
    }
}

/// .NET type name → simple type; `None` for non-system types (enums).
fn simple_type(full: &str) -> Option<Ty> {
    let t = full.split(',').next().unwrap_or("").trim();
    Some(match t {
        "System.String" | "System.Type" | "System.RuntimeType" | "System.DateTimeOffset" => Ty::Str,
        "System.Boolean" => Ty::Bool,
        "System.Byte" => Ty::Byte,
        "System.SByte" => Ty::SByte,
        "System.Char" => Ty::Char,
        "System.Int16" => Ty::I16,
        "System.UInt16" => Ty::U16,
        "System.Int32" => Ty::I32,
        "System.UInt32" => Ty::U32,
        "System.Int64" | "System.DateTime" | "System.TimeSpan" => Ty::I64,
        "System.UInt64" => Ty::U64,
        "System.Single" => Ty::F32,
        "System.Double" => Ty::F64,
        "System.Decimal" => Ty::Decimal,
        "System.Guid" => Ty::Guid,
        "System.Byte[]" => Ty::Bytes,
        _ => return None,
    })
}

/// Declared type from the member name (Recursion pack members).
fn guess(name: &str) -> Option<Ty> {
    let n = name.to_ascii_lowercase();
    let n = crate::pakdoc::normalize(&n).to_owned();
    // members of the Recursion classes (PAKFormat.guid is a System.Guid)
    if n == "guid" || n.ends_with("guid") {
        return Some(Ty::Guid);
    }
    if n.contains("enabled") || n.starts_with("is") || n.starts_with("has") || n == "dynamicsounds"
    {
        return Some(Ty::Bool);
    }
    if n.contains("image") || n.contains("data") || n.contains("bytes") {
        return Some(Ty::Bytes);
    }
    if n.contains("name")
        || n.contains("path")
        || n.contains("file")
        || n.contains("description")
        || n.contains("author")
        || n == "key"
        || n.contains("type")
        || n.contains("assembly")
        || n.contains("version")
    {
        return Some(Ty::Str);
    }
    None
}

/// Writer for tests: same layout as SharpSerializer's burst writer.
#[doc(hidden)]
pub mod writer {
    #[derive(Default)]
    pub struct W {
        pub buf: Vec<u8>,
    }

    impl W {
        pub fn u8(&mut self, b: u8) {
            self.buf.push(b);
        }
        pub fn string(&mut self, s: &str) {
            let mut n = s.len();
            loop {
                let b = (n & 0x7f) as u8;
                n >>= 7;
                if n == 0 {
                    self.u8(b);
                    break;
                }
                self.u8(b | 0x80);
            }
            self.buf.extend_from_slice(s.as_bytes());
        }
        pub fn opt(&mut self, s: Option<&str>) {
            match s {
                Some(s) => {
                    self.u8(1);
                    self.string(s);
                }
                None => self.u8(0),
            }
        }
        pub fn number(&mut self, n: i32) {
            if n == 0 {
                self.u8(0);
            } else if (0..=255).contains(&n) {
                self.u8(1);
                self.u8(n as u8);
            } else if (i16::MIN as i32..=i16::MAX as i32).contains(&n) {
                self.u8(2);
                self.buf.extend_from_slice(&(n as i16).to_le_bytes());
            } else {
                self.u8(4);
                self.buf.extend_from_slice(&n.to_le_bytes());
            }
        }
        pub fn header(&mut self, id: u8, name: Option<&str>, ty: Option<&str>) {
            self.u8(id);
            self.opt(name);
            self.opt(ty);
        }
        /// Complex object start; then `count` properties follow.
        pub fn complex(&mut self, name: Option<&str>, ty: Option<&str>, count: i32) {
            self.header(2, name, ty);
            self.number(count);
        }
        /// Collection start without properties; then `count` items follow.
        pub fn collection(
            &mut self,
            name: Option<&str>,
            ty: Option<&str>,
            elem: Option<&str>,
            count: i32,
        ) {
            self.header(1, name, ty);
            self.opt(elem);
            self.number(0);
            self.number(count);
        }
        /// Untyped string (type omitted as declared).
        pub fn str_prop(&mut self, name: Option<&str>, v: &str) {
            self.header(6, name, None);
            self.u8(1);
            self.string(v);
        }
        pub fn bool_prop(&mut self, name: &str, v: bool) {
            self.header(6, Some(name), None);
            self.u8(1);
            self.u8(u8::from(v));
        }
        pub fn bytes_prop(&mut self, name: Option<&str>, ty: Option<&str>, b: &[u8]) {
            self.header(6, name, ty);
            self.u8(1);
            self.number(b.len() as i32);
            self.buf.extend_from_slice(b);
        }
        /// Untyped System.Guid: presence flag + 16 raw bytes.
        pub fn guid_prop(&mut self, name: &str, g: [u8; 16]) {
            self.header(6, Some(name), None);
            self.u8(1);
            self.buf.extend_from_slice(&g);
        }
        /// Simple value with an explicit .NET type name and raw data.
        pub fn typed_raw(&mut self, name: &str, ty: &str, raw: &[u8]) {
            self.header(6, Some(name), Some(ty));
            self.u8(1);
            self.buf.extend_from_slice(raw);
        }
        pub fn int_prop(&mut self, name: &str, v: i32) {
            self.header(6, Some(name), None);
            self.u8(1);
            self.buf.extend_from_slice(&v.to_le_bytes());
        }
        /// Dictionary start (no properties); then `count` key/value pairs.
        pub fn dictionary(
            &mut self,
            name: Option<&str>,
            key: Option<&str>,
            value: Option<&str>,
            count: i32,
        ) {
            self.header(3, name, None);
            self.opt(key);
            self.opt(value);
            self.number(0);
            self.number(count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::writer::W;
    use super::*;

    fn sample() -> Vec<u8> {
        let mut w = W::default();
        w.complex(Some("Root"), Some("Test.Root, Lib"), 4);
        w.str_prop(Some("Name"), "hello");
        w.bool_prop("soundEnabled", true);
        w.int_prop("mysteryNumber", 77);
        w.collection(Some("objects"), None, Some("System.Object, mscorlib"), 2);
        w.bytes_prop(None, Some("System.Byte[], mscorlib"), b"OggS1234");
        w.complex(None, Some("Test.Inner, Lib"), 1);
        w.str_prop(Some("key"), "HEAD_SHOT");
        w.buf
    }

    #[test]
    fn parses_tree_with_inferred_types() {
        let d = parse(&sample(), 1 << 20).unwrap();
        assert!(looks_like_sharp(&sample()));
        let root = &d.root;
        assert_eq!(d.str(d.field(root, "name").unwrap()), Some("hello"));
        assert_eq!(d.field(root, "soundEnabled"), Some(&Node::Bool(true)));
        assert_eq!(d.field(root, "mysteryNumber"), Some(&Node::Int(77)));
        let items = d.list(d.field(root, "objects").unwrap());
        assert_eq!(items[0], &Node::Bytes(b"OggS1234".to_vec()));
        assert_eq!(d.str(d.field(items[1], "key").unwrap()), Some("HEAD_SHOT"));
    }

    /// Regression (Deadpool pack): an untyped `guid` member with 16 raw bytes
    /// between the sound entries and `componentInfo` (bytes rebuilt by hand).
    #[test]
    fn untyped_guid_before_component_info() {
        let mut w = W::default();
        w.complex(
            Some("Root"),
            Some("RecursionTracker.PAKFormat, RTLibrary"),
            3,
        );
        w.str_prop(Some("pakSoundFile"), "dGFrZSBub3Rlcy5vZ2dfU3lzdGVtLkJ5dGU=");
        w.guid_prop("guid", [0; 16]);
        w.complex(Some("componentInfo"), None, 1);
        w.str_prop(Some("name"), "RGVhZHBvb2wgdjI=");
        let d = parse(&w.buf, 1 << 20).unwrap();
        assert_eq!(d.field(&d.root, "guid"), Some(&Node::Bytes(vec![0; 16])));
        let ci = d.field(&d.root, "componentInfo").unwrap();
        assert_eq!(
            d.str(d.field(ci, "name").unwrap()),
            Some("RGVhZHBvb2wgdjI=")
        );
        // same with an unknown member name: found by the candidates
        let mut w2 = W::default();
        w2.complex(Some("Root"), None, 2);
        w2.guid_prop("someId", [7; 16]);
        w2.complex(Some("componentInfo"), None, 0);
        let d2 = parse(&w2.buf, 1 << 20).unwrap();
        assert_eq!(
            d2.field(&d2.root, "someId"),
            Some(&Node::Bytes(vec![7; 16]))
        );
    }

    /// Every simple type exactly as SharpSerializer writes it
    /// (BinaryReaderTools.readValueCore: BinaryReader primitives, Guid 16 bytes,
    /// Decimal 16 bytes, DateTime/TimeSpan Int64 ticks, enum Int32, Type string).
    #[test]
    fn all_simple_types() {
        let mut w = W::default();
        let vals: Vec<(&str, &str, Vec<u8>, Node)> = vec![
            (
                "s",
                "System.String",
                vec![2, b'h', b'i'],
                Node::Str("hi".into()),
            ),
            ("b", "System.Boolean", vec![1], Node::Bool(true)),
            ("u8", "System.Byte", vec![200], Node::Int(200)),
            ("i8", "System.SByte", vec![0xff], Node::Int(-1)),
            ("c", "System.Char", vec![b'x'], Node::Str("x".into())),
            (
                "i16",
                "System.Int16",
                (-2i16).to_le_bytes().to_vec(),
                Node::Int(-2),
            ),
            (
                "u16",
                "System.UInt16",
                65000u16.to_le_bytes().to_vec(),
                Node::Int(65000),
            ),
            (
                "i32",
                "System.Int32",
                (-3i32).to_le_bytes().to_vec(),
                Node::Int(-3),
            ),
            (
                "u32",
                "System.UInt32",
                4_000_000_000u32.to_le_bytes().to_vec(),
                Node::Int(4_000_000_000),
            ),
            (
                "i64",
                "System.Int64",
                (-5i64).to_le_bytes().to_vec(),
                Node::Int(-5),
            ),
            (
                "u64",
                "System.UInt64",
                6u64.to_le_bytes().to_vec(),
                Node::Int(6),
            ),
            (
                "f32",
                "System.Single",
                1.5f32.to_le_bytes().to_vec(),
                Node::Float(1.5),
            ),
            (
                "f64",
                "System.Double",
                2.25f64.to_le_bytes().to_vec(),
                Node::Float(2.25),
            ),
            ("dec", "System.Decimal", vec![0; 16], Node::Float(0.0)),
            (
                "dt",
                "System.DateTime",
                7i64.to_le_bytes().to_vec(),
                Node::Int(7),
            ),
            (
                "ts",
                "System.TimeSpan",
                8i64.to_le_bytes().to_vec(),
                Node::Int(8),
            ),
            ("g", "System.Guid", vec![9; 16], Node::Bytes(vec![9; 16])),
            (
                "e",
                "RecursionTracker.SomeEnum, RTLibrary",
                3i32.to_le_bytes().to_vec(),
                Node::Int(3),
            ),
            (
                "t",
                "System.Type",
                vec![3, b'a', b'b', b'c'],
                Node::Str("abc".into()),
            ),
            (
                "bytes",
                "System.Byte[]",
                vec![1, 2, 0xaa, 0xbb],
                Node::Bytes(vec![0xaa, 0xbb]),
            ),
        ];
        w.complex(Some("Root"), None, vals.len() as i32);
        for (n, ty, raw, _) in &vals {
            w.typed_raw(n, ty, raw);
        }
        let d = parse(&w.buf, 1 << 20).unwrap();
        for (n, _, _, want) in &vals {
            assert_eq!(d.field(&d.root, n), Some(want), "{n}");
        }
    }

    #[test]
    fn fuzz_no_panic() {
        let seed = sample();
        let mut rng = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        for i in 0..100_000 {
            let mut d = seed.clone();
            match i % 3 {
                0 => d.truncate((next() as usize) % (d.len() + 1)),
                1 => {
                    let p = (next() as usize) % d.len();
                    d[p] = next() as u8;
                }
                _ => d = (0..(next() % 48) as usize).map(|_| next() as u8).collect(),
            }
            let _ = parse(&d, 1 << 20);
        }
    }
}
