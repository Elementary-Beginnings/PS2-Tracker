//! Format-neutral tree for the binary Recursion pack variants (SharpSerializer,
//! .NET BinaryFormatter, protobuf). The readers produce it, the PAK mapper
//! (`recursion_pak`) walks it. Plain data only.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Node {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<Node>),
    Map(Vec<(Node, Node)>),
    Obj {
        ty: String,
        fields: Vec<(String, Node)>,
    },
    /// Shared object, see [`Doc::targets`].
    Ref(i64),
}

#[derive(Debug, Default)]
pub struct Doc {
    pub root: Node,
    pub targets: HashMap<i64, Node>,
}

/// Maximum reference chain length when resolving.
const MAX_HOPS: usize = 16;

impl Doc {
    /// Follows references (bounded, so cycles end).
    pub fn resolve<'a>(&'a self, mut n: &'a Node) -> &'a Node {
        for _ in 0..MAX_HOPS {
            match n {
                Node::Ref(id) => match self.targets.get(id) {
                    Some(t) => n = t,
                    None => return &Node::Null,
                },
                _ => return n,
            }
        }
        &Node::Null
    }

    pub fn str<'a>(&'a self, n: &'a Node) -> Option<&'a str> {
        match self.resolve(n) {
            Node::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Field of an object, case-insensitive; .NET auto properties
    /// (`<Name>k__BackingField`) and `_name` fields count as `name`.
    pub fn field<'a>(&'a self, n: &'a Node, name: &str) -> Option<&'a Node> {
        let Node::Obj { fields, .. } = self.resolve(n) else {
            return None;
        };
        fields
            .iter()
            .find(|(k, _)| normalize(k).eq_ignore_ascii_case(name))
            .map(|(_, v)| self.resolve(v))
    }

    /// Elements of a list, an array, or a .NET `List<T>` / `ArrayList` object
    /// (`_items` limited by `_size`).
    pub fn list<'a>(&'a self, n: &'a Node) -> Vec<&'a Node> {
        match self.resolve(n) {
            Node::List(v) => v.iter().map(|x| self.resolve(x)).collect(),
            o @ Node::Obj { .. } => {
                let Some(items) = self.field(o, "items") else {
                    return Vec::new();
                };
                let mut v = self.list(items);
                if let Some(Node::Int(size)) = self.field(o, "size") {
                    v.truncate(usize::try_from(*size).unwrap_or(0));
                }
                v
            }
            _ => Vec::new(),
        }
    }

    /// Key/value pairs of a map or a .NET dictionary (`KeyValuePairs`, or
    /// `Keys`/`Values` as with `Hashtable`).
    pub fn pairs<'a>(&'a self, n: &'a Node) -> Vec<(&'a Node, &'a Node)> {
        match self.resolve(n) {
            Node::Map(v) => v
                .iter()
                .map(|(k, v)| (self.resolve(k), self.resolve(v)))
                .collect(),
            o @ Node::Obj { .. } => {
                if let Some(kv) = self.field(o, "KeyValuePairs") {
                    return self
                        .list(kv)
                        .into_iter()
                        .filter_map(|p| Some((self.field(p, "key")?, self.field(p, "value")?)))
                        .collect();
                }
                match (self.field(o, "Keys"), self.field(o, "Values")) {
                    (Some(k), Some(v)) => self.list(k).into_iter().zip(self.list(v)).collect(),
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    /// Visits every node once (depth first, bounded depth, shared objects once).
    pub fn walk<'a>(&'a self, mut f: impl FnMut(&'a Node, Option<&'a Node>)) {
        let mut seen = std::collections::HashSet::new();
        let mut stack: Vec<(&'a Node, Option<&'a Node>, usize)> = vec![(&self.root, None, 0)];
        while let Some((n, key, depth)) = stack.pop() {
            if let Node::Ref(id) = n
                && !seen.insert(*id)
            {
                continue;
            }
            let n = self.resolve(n);
            f(n, key);
            if depth >= 64 {
                continue;
            }
            match n {
                Node::List(v) => stack.extend(v.iter().map(|x| (x, None, depth + 1))),
                Node::Map(v) => {
                    for (k, val) in v {
                        stack.push((k, None, depth + 1));
                        stack.push((val, Some(k), depth + 1));
                    }
                }
                Node::Obj { fields, .. } => {
                    stack.extend(fields.iter().map(|(_, x)| (x, None, depth + 1)))
                }
                _ => {}
            }
        }
    }
}

/// `<Name>k__BackingField` → `Name`, `_name` / `m_name` → `name`.
pub fn normalize(k: &str) -> &str {
    if let Some(rest) = k.strip_prefix('<')
        && let Some(end) = rest.find('>')
    {
        return &rest[..end];
    }
    k.strip_prefix("m_")
        .or_else(|| k.strip_prefix('_'))
        .unwrap_or(k)
}

/// Converts a BinaryFormatter graph.
pub fn from_nrbf(g: &crate::nrbf::Graph) -> Doc {
    use crate::nrbf::{Object, Value};
    fn val(v: &Value) -> Node {
        match v {
            Value::Null => Node::Null,
            Value::Bool(b) => Node::Bool(*b),
            Value::Int(i) => Node::Int(*i),
            Value::UInt(u) => Node::Int(i64::try_from(*u).unwrap_or(i64::MAX)),
            Value::Float(f) => Node::Float(*f),
            Value::Char(c) => Node::Str(c.to_string()),
            Value::Text(s) => Node::Str(s.clone()),
            Value::Ref(id) => Node::Ref(i64::from(*id)),
        }
    }
    let mut targets = HashMap::new();
    for (id, o) in &g.objects {
        let n = match o {
            Object::Class { name, members } => Node::Obj {
                ty: name.clone(),
                fields: members.iter().map(|(k, v)| (k.clone(), val(v))).collect(),
            },
            Object::String(s) => Node::Str(s.clone()),
            Object::Bytes(b) => Node::Bytes(b.clone()),
            Object::Array(a) => Node::List(a.iter().map(val).collect()),
        };
        targets.insert(i64::from(*id), n);
    }
    Doc {
        root: Node::Ref(i64::from(g.root)),
        targets,
    }
}
