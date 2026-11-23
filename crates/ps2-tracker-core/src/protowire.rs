//! Minimal protobuf wire-format reader (protobuf.dev "Encoding"): varint,
//! 64-bit, length-delimited and 32-bit fields. No schema, no code generation:
//! a message is a list of (field number, value); whether a length-delimited
//! value is a nested message, text or bytes decides the caller.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wire<'a> {
    Varint(u64),
    I64(u64),
    Len(&'a [u8]),
    I32(u32),
}

pub type Message<'a> = Vec<(u32, Wire<'a>)>;

#[derive(Debug, PartialEq)]
pub struct WireError {
    pub offset: usize,
    pub reason: &'static str,
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "corrupt PAK record at offset {}: {}",
            self.offset, self.reason
        )
    }
}

/// Upper bound for fields of one message (against hostile input).
pub const MAX_FIELDS: usize = 1_000_000;

fn varint(d: &[u8], pos: &mut usize) -> Result<u64, WireError> {
    let start = *pos;
    let mut v = 0u64;
    for i in 0..10 {
        let Some(&b) = d.get(*pos) else {
            return Err(WireError {
                offset: start,
                reason: "truncated varint",
            });
        };
        *pos += 1;
        v |= u64::from(b & 0x7f) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err(WireError {
        offset: start,
        reason: "varint too long",
    })
}

/// Parses one message completely; every length is checked against the input.
/// `base` is the offset of `d` in the file (for error messages).
pub fn parse(d: &[u8], base: usize) -> Result<Message<'_>, WireError> {
    let mut pos = 0;
    let mut out = Vec::new();
    while pos < d.len() {
        let at = pos;
        let key = varint(d, &mut pos).map_err(|e| WireError {
            offset: base + e.offset,
            ..e
        })?;
        let field = u32::try_from(key >> 3).unwrap_or(0);
        let err = |reason| WireError {
            offset: base + at,
            reason,
        };
        if field == 0 {
            return Err(err("field number 0"));
        }
        let v = match key & 7 {
            0 => Wire::Varint(varint(d, &mut pos).map_err(|_| err("bad varint"))?),
            1 => {
                let b = d.get(pos..pos + 8).ok_or(err("truncated fixed64"))?;
                pos += 8;
                Wire::I64(u64::from_le_bytes(b.try_into().expect("8")))
            }
            2 => {
                let n = varint(d, &mut pos).map_err(|_| err("bad length"))?;
                let n = usize::try_from(n).map_err(|_| err("length too large"))?;
                if n > d.len() - pos {
                    return Err(err("length beyond the end"));
                }
                let s = &d[pos..pos + n];
                pos += n;
                Wire::Len(s)
            }
            5 => {
                let b = d.get(pos..pos + 4).ok_or(err("truncated fixed32"))?;
                pos += 4;
                Wire::I32(u32::from_le_bytes(b.try_into().expect("4")))
            }
            _ => return Err(err("unsupported wire type")),
        };
        out.push((field, v));
        if out.len() > MAX_FIELDS {
            return Err(err("too many fields"));
        }
    }
    Ok(out)
}

/// Helpers for reading a parsed message.
pub trait MessageExt<'a> {
    fn get(&self, field: u32) -> Option<Wire<'a>>;
    fn all(&self, field: u32) -> Vec<Wire<'a>>;
    fn bytes(&self, field: u32) -> Option<&'a [u8]>;
    fn text(&self, field: u32) -> Option<&'a str>;
    fn varint(&self, field: u32) -> Option<u64>;
}

impl<'a> MessageExt<'a> for Message<'a> {
    fn get(&self, field: u32) -> Option<Wire<'a>> {
        self.iter().find(|(f, _)| *f == field).map(|(_, v)| *v)
    }
    fn all(&self, field: u32) -> Vec<Wire<'a>> {
        self.iter()
            .filter(|(f, _)| *f == field)
            .map(|(_, v)| *v)
            .collect()
    }
    fn bytes(&self, field: u32) -> Option<&'a [u8]> {
        match self.get(field)? {
            Wire::Len(b) => Some(b),
            _ => None,
        }
    }
    fn text(&self, field: u32) -> Option<&'a str> {
        std::str::from_utf8(self.bytes(field)?).ok()
    }
    fn varint(&self, field: u32) -> Option<u64> {
        match self.get(field)? {
            Wire::Varint(v) => Some(v),
            _ => None,
        }
    }
}

/// Writer for tests.
#[doc(hidden)]
pub mod writer {
    pub fn varint(out: &mut Vec<u8>, mut v: u64) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                return;
            }
            out.push(b | 0x80);
        }
    }
    pub fn uint(out: &mut Vec<u8>, field: u32, v: u64) {
        varint(out, u64::from(field) << 3);
        varint(out, v);
    }
    pub fn bytes(out: &mut Vec<u8>, field: u32, b: &[u8]) {
        varint(out, (u64::from(field) << 3) | 2);
        varint(out, b.len() as u64);
        out.extend_from_slice(b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_errors() {
        let mut inner = Vec::new();
        writer::bytes(&mut inner, 1, b"HEAD_SHOT");
        writer::uint(&mut inner, 2, 1);
        let mut m = Vec::new();
        writer::bytes(&mut m, 1, &inner);
        writer::uint(&mut m, 4, 300);
        let msg = parse(&m, 0).unwrap();
        assert_eq!(msg.varint(4), Some(300));
        let sub = parse(msg.bytes(1).unwrap(), 0).unwrap();
        assert_eq!(sub.text(1), Some("HEAD_SHOT"));
        assert!(parse(&m[..m.len() - 1], 0).is_err());
        assert!(parse(&[0x0a, 0xff, 0xff, 0xff, 0xff, 0x0f], 0).is_err());
    }

    #[test]
    fn fuzz_no_panic() {
        let mut rng = 0x1234_5678_9abc_def1u64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        for _ in 0..200_000 {
            let n = (next() % 64) as usize;
            let d: Vec<u8> = (0..n).map(|_| next() as u8).collect();
            if let Ok(m) = parse(&d, 0) {
                for (_, w) in m {
                    if let Wire::Len(b) = w {
                        let _ = parse(b, 0);
                    }
                }
            }
        }
    }
}
