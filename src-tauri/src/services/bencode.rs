//! Minimal bencode (BEP 3) reader and writer.
//!
//! Enough to open a `.torrent`, change one field and write it back. Dictionaries
//! are kept in a `BTreeMap`, so encoding is always canonical (keys sorted,
//! bytewise) — a well-formed torrent round-trips byte for byte, and a caller
//! can prove that before trusting its own edit.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Dict(BTreeMap<Vec<u8>, Value>),
}

impl Value {
    pub fn as_dict(&self) -> Option<&BTreeMap<Vec<u8>, Value>> {
        match self {
            Value::Dict(d) => Some(d),
            _ => None,
        }
    }

    pub fn as_dict_mut(&mut self) -> Option<&mut BTreeMap<Vec<u8>, Value>> {
        match self {
            Value::Dict(d) => Some(d),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(l) => Some(l),
            _ => None,
        }
    }
}

/// Decode one complete value; trailing bytes are an error.
pub fn decode(data: &[u8]) -> Result<Value, String> {
    let (value, used) = decode_at(data, 0, 0)?;
    if used != data.len() {
        return Err(format!(
            "{} trailing bytes after the value",
            data.len() - used
        ));
    }
    Ok(value)
}

const MAX_DEPTH: usize = 64;

fn decode_at(data: &[u8], pos: usize, depth: usize) -> Result<(Value, usize), String> {
    if depth > MAX_DEPTH {
        return Err("nesting too deep".into());
    }
    match data.get(pos) {
        Some(b'i') => {
            let end = find(data, pos + 1, b'e')?;
            let text = std::str::from_utf8(&data[pos + 1..end]).map_err(|_| "bad integer")?;
            if text == "-0" || (text.len() > 1 && (text.starts_with('0') || text.starts_with("-0")))
            {
                return Err(format!("non-canonical integer {text:?}"));
            }
            let n: i64 = text.parse().map_err(|_| format!("bad integer {text:?}"))?;
            Ok((Value::Int(n), end + 1))
        }
        Some(b'l') => {
            let mut items = Vec::new();
            let mut p = pos + 1;
            while data.get(p) != Some(&b'e') {
                if p >= data.len() {
                    return Err("unterminated list".into());
                }
                let (v, next) = decode_at(data, p, depth + 1)?;
                items.push(v);
                p = next;
            }
            Ok((Value::List(items), p + 1))
        }
        Some(b'd') => {
            let mut map = BTreeMap::new();
            let mut last_key: Option<Vec<u8>> = None;
            let mut p = pos + 1;
            while data.get(p) != Some(&b'e') {
                if p >= data.len() {
                    return Err("unterminated dictionary".into());
                }
                let (k, next) = decode_at(data, p, depth + 1)?;
                let Value::Bytes(key) = k else {
                    return Err("dictionary key is not a string".into());
                };
                if last_key.as_ref().is_some_and(|prev| *prev >= key) {
                    return Err("dictionary keys out of order".into());
                }
                let (v, next) = decode_at(data, next, depth + 1)?;
                last_key = Some(key.clone());
                map.insert(key, v);
                p = next;
            }
            Ok((Value::Dict(map), p + 1))
        }
        Some(b'0'..=b'9') => {
            let colon = find(data, pos, b':')?;
            let text = std::str::from_utf8(&data[pos..colon]).map_err(|_| "bad length")?;
            if text.len() > 1 && text.starts_with('0') {
                return Err(format!("non-canonical length {text:?}"));
            }
            let len: usize = text.parse().map_err(|_| format!("bad length {text:?}"))?;
            let start = colon + 1;
            let end = start
                .checked_add(len)
                .filter(|&e| e <= data.len())
                .ok_or("string runs past the end")?;
            Ok((Value::Bytes(data[start..end].to_vec()), end))
        }
        Some(other) => Err(format!("unexpected byte {other:#04x} at {pos}")),
        None => Err("unexpected end of data".into()),
    }
}

fn find(data: &[u8], from: usize, byte: u8) -> Result<usize, String> {
    data[from.min(data.len())..]
        .iter()
        .position(|&b| b == byte)
        .map(|i| from + i)
        .ok_or_else(|| format!("missing {:?}", byte as char))
}

pub fn encode(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(value, &mut out);
    out
}

fn encode_into(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Int(n) => {
            out.push(b'i');
            out.extend_from_slice(n.to_string().as_bytes());
            out.push(b'e');
        }
        Value::Bytes(b) => {
            out.extend_from_slice(b.len().to_string().as_bytes());
            out.push(b':');
            out.extend_from_slice(b);
        }
        Value::List(items) => {
            out.push(b'l');
            for item in items {
                encode_into(item, out);
            }
            out.push(b'e');
        }
        Value::Dict(map) => {
            out.push(b'd');
            for (k, v) in map {
                encode_into(&Value::Bytes(k.clone()), out);
                encode_into(v, out);
            }
            out.push(b'e');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict(pairs: &[(&str, Value)]) -> Value {
        Value::Dict(
            pairs
                .iter()
                .map(|(k, v)| (k.as_bytes().to_vec(), v.clone()))
                .collect(),
        )
    }

    #[test]
    fn round_trips_a_torrent_shaped_value() {
        let v = dict(&[
            ("announce", Value::Bytes(b"udp://t:1".to_vec())),
            (
                "info",
                dict(&[
                    (
                        "files",
                        Value::List(vec![dict(&[
                            ("length", Value::Int(191)),
                            (
                                "path",
                                Value::List(vec![Value::Bytes(b"beanfun.url".to_vec())]),
                            ),
                        ])]),
                    ),
                    ("name", Value::Bytes(b"Bin64".to_vec())),
                    ("piece length", Value::Int(4194304)),
                    ("pieces", Value::Bytes(vec![0u8; 40])),
                ]),
            ),
            (
                "url-list",
                Value::List(vec![Value::Bytes(b"https://x/".to_vec())]),
            ),
        ]);
        let bytes = encode(&v);
        assert_eq!(decode(&bytes).unwrap(), v);
        assert_eq!(encode(&decode(&bytes).unwrap()), bytes);
    }

    #[test]
    fn decodes_the_spec_examples() {
        assert_eq!(decode(b"i-3e").unwrap(), Value::Int(-3));
        assert_eq!(decode(b"4:spam").unwrap(), Value::Bytes(b"spam".to_vec()));
        assert_eq!(
            decode(b"l4:spam4:eggse").unwrap(),
            Value::List(vec![
                Value::Bytes(b"spam".to_vec()),
                Value::Bytes(b"eggs".to_vec())
            ])
        );
        assert_eq!(
            decode(b"d3:cow3:moo4:spam4:eggse").unwrap(),
            dict(&[
                ("cow", Value::Bytes(b"moo".to_vec())),
                ("spam", Value::Bytes(b"eggs".to_vec()))
            ])
        );
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in [
            &b"i03e"[..],
            b"i-0e",
            b"03:abc",
            b"5:abc",
            b"l4:spam",
            b"d4:spam4:eggs3:cow3:mooe", // keys out of order
            b"di1e4:spame",              // non-string key
            b"4:spamx",                  // trailing byte
            b"",
            b"x",
        ] {
            assert!(decode(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn deep_nesting_is_refused_rather_than_overflowing() {
        let mut data = vec![b'l'; 10_000];
        data.extend(std::iter::repeat_n(b'e', 10_000));
        assert!(decode(&data).is_err());
    }
}
