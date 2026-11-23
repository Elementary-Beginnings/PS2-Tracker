//! Reader for the .NET Remoting Binary Format ([MS-NRBF], "BinaryFormatter").
//!
//! Purely data driven: the records are read into a graph of plain values. No type
//! is ever looked up or instantiated; class and member names are just strings.
//! Hard limits against hostile input: total size, single array size, number of
//! records, nesting depth and member count; every length is checked against the
//! remaining input before anything is allocated. References are resolved by
//! object id only when the caller walks the graph (with its own depth limit).
//!
//! Implemented records (MS-NRBF 2.1.2.1 RecordTypeEnumeration): 0
//! SerializedStreamHeader, 1 ClassWithId, 2 SystemClassWithMembers, 3
//! ClassWithMembers, 4 SystemClassWithMembersAndTypes, 5 ClassWithMembersAndTypes,
//! 6 BinaryObjectString, 7 BinaryArray, 8 MemberPrimitiveTyped, 9 MemberReference,
//! 10 ObjectNull, 11 MessageEnd, 12 BinaryLibrary, 13 ObjectNullMultiple256,
//! 14 ObjectNullMultiple, 15 ArraySinglePrimitive, 16 ArraySingleObject,
//! 17 ArraySingleString. Method call/return records (21, 22) are rejected.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_total: usize,
    pub max_array_bytes: usize,
    pub max_records: usize,
    pub max_depth: usize,
    pub max_members: usize,
    /// Elements of all arrays together (null runs are cheap to encode).
    pub max_elements: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_total: 256 * 1024 * 1024,
            max_array_bytes: 32 * 1024 * 1024,
            max_records: 1_000_000,
            max_depth: 64,
            max_members: 4096,
            max_elements: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NrbfError {
    TooLarge { size: usize, limit: usize },
    Corrupt { offset: usize, reason: String },
}

impl std::fmt::Display for NrbfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { size, limit } => write!(
                f,
                "too large ({:.1} MB, limit {:.0} MB)",
                *size as f64 / 1048576.0,
                *limit as f64 / 1048576.0
            ),
            Self::Corrupt { offset, reason } => {
                write!(f, "corrupt PAK record at offset {offset}: {reason}")
            }
        }
    }
}

/// A member or array element.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Char(char),
    /// Primitive string/decimal written inline (not an object).
    Text(String),
    /// Reference to an object in [`Graph::objects`].
    Ref(i32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Object {
    Class {
        name: String,
        members: Vec<(String, Value)>,
    },
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
}

#[derive(Debug, Default)]
pub struct Graph {
    pub root: i32,
    pub objects: HashMap<i32, Object>,
}

impl Graph {
    pub fn get(&self, v: &Value) -> Option<&Object> {
        match v {
            Value::Ref(id) => self.objects.get(id),
            _ => None,
        }
    }
    pub fn root(&self) -> Option<&Object> {
        self.objects.get(&self.root)
    }
    /// The string behind a value (object string or inline text).
    pub fn string<'a>(&'a self, v: &'a Value) -> Option<&'a str> {
        match v {
            Value::Text(s) => Some(s),
            Value::Ref(id) => match self.objects.get(id)? {
                Object::String(s) => Some(s),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Starts like a BinaryFormatter stream (SerializedStreamHeader record)?
pub fn looks_like_nrbf(head: &[u8]) -> bool {
    head.len() >= 17 && head[0] == 0 && head[9..17] == [1, 0, 0, 0, 0, 0, 0, 0]
}

#[derive(Clone)]
struct ClassMeta {
    name: String,
    members: Vec<String>,
    /// `None`: untyped (ClassWithMembers / SystemClassWithMembers)
    types: Option<Vec<MemberType>>,
}

#[derive(Clone, Copy, Debug)]
enum MemberType {
    Primitive(u8),
    /// String, Object, SystemClass, Class, ObjectArray, StringArray, PrimitiveArray:
    /// all are written as records.
    Record,
}

enum Item {
    Value(Value),
    Nulls(usize),
    End,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    limits: Limits,
    records: usize,
    elements: usize,
    metas: HashMap<i32, ClassMeta>,
    graph: Graph,
}

pub fn parse(data: &[u8], limits: Limits) -> Result<Graph, NrbfError> {
    if data.len() > limits.max_total {
        return Err(NrbfError::TooLarge {
            size: data.len(),
            limit: limits.max_total,
        });
    }
    let mut r = Reader {
        data,
        pos: 0,
        limits,
        records: 0,
        elements: 0,
        metas: HashMap::new(),
        graph: Graph::default(),
    };
    if r.u8()? != 0 {
        return Err(r.err_at(0, "missing SerializedStreamHeader"));
    }
    r.graph.root = r.i32()?;
    let _header_id = r.i32()?;
    let (major, minor) = (r.i32()?, r.i32()?);
    if (major, minor) != (1, 0) {
        return Err(r.err_at(9, format!("unsupported version {major}.{minor}")));
    }
    loop {
        match r.record(0)? {
            Item::End => break,
            Item::Value(_) | Item::Nulls(_) => {}
        }
    }
    if !r.graph.objects.contains_key(&r.graph.root) {
        return Err(r.err("root object missing"));
    }
    Ok(r.graph)
}

impl<'a> Reader<'a> {
    fn err(&self, reason: impl Into<String>) -> NrbfError {
        self.err_at(self.pos, reason)
    }
    fn err_at(&self, offset: usize, reason: impl Into<String>) -> NrbfError {
        NrbfError::Corrupt {
            offset,
            reason: reason.into(),
        }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], NrbfError> {
        if n > self.data.len() - self.pos {
            return Err(self.err(format!(
                "needs {n} bytes, only {} left",
                self.data.len() - self.pos
            )));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, NrbfError> {
        Ok(self.take(1)?[0])
    }
    fn i32(&mut self) -> Result<i32, NrbfError> {
        Ok(i32::from_le_bytes(
            self.take(4)?.try_into().expect("4 bytes"),
        ))
    }
    fn count(&mut self, what: &str) -> Result<usize, NrbfError> {
        let at = self.pos;
        let n = self.i32()?;
        usize::try_from(n).map_err(|_| self.err_at(at, format!("negative {what}")))
    }
    fn lps(&mut self) -> Result<String, NrbfError> {
        let at = self.pos;
        let mut len: usize = 0;
        for i in 0..5 {
            let b = self.u8()?;
            len |= usize::from(b & 0x7f) << (7 * i);
            if b & 0x80 == 0 {
                break;
            }
            if i == 4 {
                return Err(self.err_at(at, "string length too long"));
            }
        }
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| self.err_at(at, "string is not UTF-8"))
    }

    fn primitive(&mut self, ty: u8) -> Result<Value, NrbfError> {
        let at = self.pos;
        let le8 = |r: &mut Self| -> Result<[u8; 8], NrbfError> {
            Ok(r.take(8)?.try_into().expect("8 bytes"))
        };
        Ok(match ty {
            1 => Value::Bool(self.u8()? != 0),
            2 => Value::UInt(u64::from(self.u8()?)),
            3 => {
                let first = self.u8()?;
                let n = match first {
                    0x00..=0x7f => 1,
                    0xc0..=0xdf => 2,
                    0xe0..=0xef => 3,
                    0xf0..=0xf7 => 4,
                    _ => return Err(self.err_at(at, "invalid char")),
                };
                let mut buf = vec![first];
                buf.extend_from_slice(self.take(n - 1)?);
                let s = std::str::from_utf8(&buf).map_err(|_| self.err_at(at, "invalid char"))?;
                Value::Char(s.chars().next().unwrap_or('\0'))
            }
            5 | 18 => Value::Text(self.lps()?),
            6 => Value::Float(f64::from_le_bytes(le8(self)?)),
            7 => Value::Int(i64::from(i16::from_le_bytes(
                self.take(2)?.try_into().expect("2"),
            ))),
            8 => Value::Int(i64::from(self.i32()?)),
            9 | 12 | 13 => Value::Int(i64::from_le_bytes(le8(self)?)),
            10 => Value::Int(i64::from(self.u8()? as i8)),
            11 => Value::Float(f64::from(f32::from_le_bytes(
                self.take(4)?.try_into().expect("4"),
            ))),
            14 => Value::UInt(u64::from(u16::from_le_bytes(
                self.take(2)?.try_into().expect("2"),
            ))),
            15 => Value::UInt(u64::from(u32::from_le_bytes(
                self.take(4)?.try_into().expect("4"),
            ))),
            16 => Value::UInt(u64::from_le_bytes(le8(self)?)),
            17 => Value::Null,
            _ => return Err(self.err_at(at, format!("unknown primitive type {ty}"))),
        })
    }

    /// Fixed size of a primitive (for array length checks), 1 if variable.
    fn primitive_size(ty: u8) -> usize {
        match ty {
            7 | 14 => 2,
            8 | 11 | 15 => 4,
            6 | 9 | 12 | 13 | 16 => 8,
            _ => 1,
        }
    }

    fn insert(&mut self, at: usize, id: i32, obj: Object) -> Result<Value, NrbfError> {
        if self.graph.objects.insert(id, obj).is_some() {
            return Err(self.err_at(at, format!("object id {id} defined twice")));
        }
        Ok(Value::Ref(id))
    }

    fn class_info(&mut self) -> Result<(i32, String, Vec<String>), NrbfError> {
        let id = self.i32()?;
        let name = self.lps()?;
        let n = self.count("member count")?;
        if n > self.limits.max_members {
            return Err(self.err(format!("{n} members")));
        }
        let mut members = Vec::with_capacity(n.min(64));
        for _ in 0..n {
            members.push(self.lps()?);
        }
        Ok((id, name, members))
    }

    fn member_types(&mut self, n: usize) -> Result<Vec<MemberType>, NrbfError> {
        let kinds = self.take(n)?.to_vec();
        let mut out = Vec::with_capacity(n);
        for k in kinds {
            out.push(match k {
                0 => MemberType::Primitive(self.u8()?),
                7 => {
                    self.u8()?;
                    MemberType::Record
                }
                1 | 2 | 5 | 6 => MemberType::Record,
                3 => {
                    self.lps()?;
                    MemberType::Record
                }
                4 => {
                    self.lps()?;
                    self.i32()?;
                    MemberType::Record
                }
                _ => return Err(self.err(format!("unknown member type {k}"))),
            });
        }
        Ok(out)
    }

    fn class_values(
        &mut self,
        at: usize,
        id: i32,
        meta: ClassMeta,
        depth: usize,
    ) -> Result<Value, NrbfError> {
        let mut members = Vec::with_capacity(meta.members.len().min(64));
        let mut nulls = 0usize;
        for (i, name) in meta.members.iter().enumerate() {
            let ty = meta.types.as_ref().map_or(MemberType::Record, |t| t[i]);
            let v = match ty {
                MemberType::Primitive(p) => self.primitive(p)?,
                MemberType::Record if nulls > 0 => {
                    nulls -= 1;
                    Value::Null
                }
                MemberType::Record => match self.record(depth + 1)? {
                    Item::Value(v) => v,
                    Item::Nulls(n) => {
                        nulls = n - 1;
                        Value::Null
                    }
                    Item::End => return Err(self.err("MessageEnd inside a class")),
                },
            };
            members.push((name.clone(), v));
        }
        if nulls > 0 {
            return Err(self.err("null run longer than the members"));
        }
        self.insert(
            at,
            id,
            Object::Class {
                name: meta.name,
                members,
            },
        )
    }

    fn elements(&mut self, n: usize, depth: usize) -> Result<Vec<Value>, NrbfError> {
        self.elements += n;
        if self.elements > self.limits.max_elements {
            return Err(self.err("too many array elements"));
        }
        let mut out = Vec::with_capacity(n.min(4096));
        while out.len() < n {
            match self.record(depth + 1)? {
                Item::Value(v) => out.push(v),
                Item::Nulls(k) => {
                    if k > n - out.len() {
                        return Err(self.err("null run longer than the array"));
                    }
                    out.extend(std::iter::repeat_n(Value::Null, k));
                }
                Item::End => return Err(self.err("MessageEnd inside an array")),
            }
        }
        Ok(out)
    }

    fn record(&mut self, depth: usize) -> Result<Item, NrbfError> {
        if depth > self.limits.max_depth {
            return Err(self.err("nesting too deep"));
        }
        self.records += 1;
        if self.records > self.limits.max_records {
            return Err(self.err("too many records"));
        }
        let mut at = self.pos;
        let mut kind = self.u8()?;
        while kind == 12 {
            // BinaryLibrary: precedes the record that uses it
            self.i32()?;
            self.lps()?;
            at = self.pos;
            kind = self.u8()?;
        }
        match kind {
            1 => {
                let id = self.i32()?;
                let meta_id = self.i32()?;
                let meta = self
                    .metas
                    .get(&meta_id)
                    .cloned()
                    .ok_or_else(|| self.err_at(at, format!("unknown metadata id {meta_id}")))?;
                self.class_values(at, id, meta, depth).map(Item::Value)
            }
            2..=5 => {
                let (id, name, members) = self.class_info()?;
                let types = if kind >= 4 {
                    Some(self.member_types(members.len())?)
                } else {
                    None
                };
                if kind == 3 || kind == 5 {
                    self.i32()?; // library id
                }
                let meta = ClassMeta {
                    name,
                    members,
                    types,
                };
                self.metas.insert(id, meta.clone());
                self.class_values(at, id, meta, depth).map(Item::Value)
            }
            6 => {
                let id = self.i32()?;
                let s = self.lps()?;
                self.insert(at, id, Object::String(s)).map(Item::Value)
            }
            7 => {
                let id = self.i32()?;
                let array_kind = self.u8()?;
                let rank = self.count("rank")?;
                if rank == 0 || rank > 8 {
                    return Err(self.err_at(at, format!("array rank {rank}")));
                }
                let mut total = 1usize;
                for _ in 0..rank {
                    let l = self.count("array length")?;
                    total = total
                        .checked_mul(l)
                        .ok_or_else(|| self.err_at(at, "array too large"))?;
                }
                if (3..=5).contains(&array_kind) {
                    for _ in 0..rank {
                        self.i32()?;
                    }
                }
                let ty = self.member_types(1)?[0];
                let v = match ty {
                    MemberType::Primitive(p) => self.primitive_array(at, total, p)?,
                    MemberType::Record => Object::Array(self.elements(total, depth)?),
                };
                self.insert(at, id, v).map(Item::Value)
            }
            8 => {
                let p = self.u8()?;
                self.primitive(p).map(Item::Value)
            }
            9 => Ok(Item::Value(Value::Ref(self.i32()?))),
            10 => Ok(Item::Nulls(1)),
            11 => Ok(Item::End),
            13 => Ok(Item::Nulls(usize::from(self.u8()?).max(1))),
            14 => Ok(Item::Nulls(self.count("null count")?.max(1))),
            15 => {
                let id = self.i32()?;
                let n = self.count("array length")?;
                let p = self.u8()?;
                let v = self.primitive_array(at, n, p)?;
                self.insert(at, id, v).map(Item::Value)
            }
            16 | 17 => {
                let id = self.i32()?;
                let n = self.count("array length")?;
                let v = self.elements(n, depth)?;
                self.insert(at, id, Object::Array(v)).map(Item::Value)
            }
            _ => Err(self.err_at(at, format!("unsupported record type {kind}"))),
        }
    }

    fn primitive_array(&mut self, at: usize, n: usize, p: u8) -> Result<Object, NrbfError> {
        let need = n
            .checked_mul(Self::primitive_size(p))
            .ok_or_else(|| self.err_at(at, "array too large"))?;
        if need > self.data.len() - self.pos {
            return Err(self.err_at(at, format!("array of {n} needs more data than left")));
        }
        if p == 2 {
            if n > self.limits.max_array_bytes {
                return Err(self.err_at(at, format!("byte array of {n} bytes over the limit")));
            }
            return Ok(Object::Bytes(self.take(n)?.to_vec()));
        }
        Ok(Object::Array(
            (0..n)
                .map(|_| self.primitive(p))
                .collect::<Result<_, _>>()?,
        ))
    }
}

/// Writer for tests (and fuzzing seeds): builds streams record by record.
#[doc(hidden)]
pub mod writer {
    pub struct W {
        pub buf: Vec<u8>,
        next_id: i32,
    }

    impl Default for W {
        fn default() -> Self {
            Self::new()
        }
    }

    impl W {
        pub fn new() -> Self {
            let mut w = Self {
                buf: vec![0],
                next_id: 1,
            };
            w.i32(1); // root id
            w.i32(-1);
            w.i32(1);
            w.i32(0);
            w
        }
        pub fn id(&mut self) -> i32 {
            let i = self.next_id;
            self.next_id += 1;
            i
        }
        pub fn u8(&mut self, b: u8) {
            self.buf.push(b);
        }
        pub fn i32(&mut self, v: i32) {
            self.buf.extend_from_slice(&v.to_le_bytes());
        }
        pub fn lps(&mut self, s: &str) {
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
        pub fn library(&mut self, id: i32, name: &str) {
            self.u8(12);
            self.i32(id);
            self.lps(name);
        }
        /// ClassWithMembersAndTypes header; member kinds: 0 primitive (with type),
        /// 1 string, 2 object, 4 class, 5 object array, 7 primitive array.
        pub fn class(&mut self, id: i32, name: &str, members: &[(&str, u8, u8)], library: i32) {
            self.u8(5);
            self.i32(id);
            self.lps(name);
            self.i32(members.len() as i32);
            for (m, _, _) in members {
                self.lps(m);
            }
            for (_, k, _) in members {
                self.u8(*k);
            }
            for (_, k, extra) in members {
                match k {
                    0 | 7 => self.u8(*extra),
                    4 => {
                        self.lps("Some.Class");
                        self.i32(library);
                    }
                    _ => {}
                }
            }
            self.i32(library);
        }
        pub fn class_with_id(&mut self, id: i32, meta: i32) {
            self.u8(1);
            self.i32(id);
            self.i32(meta);
        }
        pub fn string(&mut self, id: i32, s: &str) {
            self.u8(6);
            self.i32(id);
            self.lps(s);
        }
        pub fn reference(&mut self, id: i32) {
            self.u8(9);
            self.i32(id);
        }
        pub fn null(&mut self) {
            self.u8(10);
        }
        pub fn bytes(&mut self, id: i32, b: &[u8]) {
            self.u8(15);
            self.i32(id);
            self.i32(b.len() as i32);
            self.u8(2);
            self.buf.extend_from_slice(b);
        }
        pub fn object_array(&mut self, id: i32, len: usize) {
            self.u8(16);
            self.i32(id);
            self.i32(len as i32);
        }
        pub fn end(&mut self) {
            self.u8(11);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::writer::W;
    use super::*;

    fn small() -> Vec<u8> {
        let mut w = W::new();
        w.library(2, "Lib");
        w.class(
            1,
            "Root",
            &[
                ("name", 1, 0),
                ("count", 0, 8),
                ("data", 7, 2),
                ("list", 5, 0),
            ],
            2,
        );
        w.string(3, "hello");
        w.i32(42);
        w.bytes(4, b"OggS....");
        w.object_array(5, 4);
        w.reference(3);
        w.u8(13);
        w.u8(2); // two nulls
        w.class_with_id(6, 1);
        w.string(7, "inner");
        w.i32(1);
        w.null();
        w.null();
        w.end();
        w.buf
    }

    #[test]
    fn parses_records_and_references() {
        let g = parse(&small(), Limits::default()).unwrap();
        let Some(Object::Class { name, members }) = g.root() else {
            panic!()
        };
        assert_eq!(name, "Root");
        assert_eq!(g.string(&members[0].1), Some("hello"));
        assert_eq!(members[1].1, Value::Int(42));
        assert_eq!(
            g.get(&members[2].1),
            Some(&Object::Bytes(b"OggS....".to_vec()))
        );
        let Some(Object::Array(items)) = g.get(&members[3].1) else {
            panic!()
        };
        assert_eq!(items.len(), 4);
        assert_eq!(g.string(&items[0]), Some("hello"));
        assert_eq!(items[1], Value::Null);
        assert_eq!(items[2], Value::Null);
        let Some(Object::Class { name, members }) = g.get(&items[3]) else {
            panic!()
        };
        assert_eq!(name, "Root", "ClassWithId reuses the metadata");
        assert_eq!(g.string(&members[0].1), Some("inner"));
    }

    #[test]
    fn limits_and_errors() {
        let data = small();
        let err = parse(
            &data,
            Limits {
                max_total: 10,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, NrbfError::TooLarge { .. }), "{err}");
        assert!(err.to_string().contains("too large"));
        let err = parse(
            &data,
            Limits {
                max_array_bytes: 4,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("over the limit"), "{err}");
        let err = parse(&data[..data.len() - 5], Limits::default()).unwrap_err();
        assert!(
            err.to_string().starts_with("corrupt PAK record at offset"),
            "{err}"
        );
        assert!(looks_like_nrbf(&data));
        assert!(!looks_like_nrbf(b"<?xml version"));
    }

    #[test]
    fn deep_nesting_is_rejected() {
        let mut w = W::new();
        for i in 0..200 {
            w.class(i + 1, "N", &[("child", 2, 0)], 0);
        }
        let err = parse(&w.buf, Limits::default()).unwrap_err();
        assert!(err.to_string().contains("too deep"), "{err}");
    }

    /// Random, truncated and mutated input must never panic or allocate wildly.
    #[test]
    fn fuzz_no_panic() {
        let seed = small();
        let mut rng = 0x2545_F491_4F6C_DD1Du64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let mut ok = 0;
        for i in 0..200_000 {
            let mut d = seed.clone();
            match i % 4 {
                0 => d.truncate((next() as usize) % (d.len() + 1)),
                1 => {
                    for _ in 0..1 + next() % 4 {
                        let p = (next() as usize) % d.len();
                        d[p] = next() as u8;
                    }
                }
                2 => {
                    let p = (next() as usize) % d.len();
                    let v = (next() as u32).to_le_bytes();
                    for (k, b) in v.iter().enumerate() {
                        if p + k < d.len() {
                            d[p + k] = *b;
                        }
                    }
                }
                _ => {
                    let n = (next() % 64) as usize;
                    d = (0..n).map(|_| next() as u8).collect();
                    if !d.is_empty() {
                        d[0] = 0;
                    }
                }
            }
            if parse(&d, Limits::default()).is_ok() {
                ok += 1;
            }
        }
        assert!(ok > 0);
    }
}
