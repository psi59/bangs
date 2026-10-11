//! A small JSON reader/writer that reproduces encoding/json semantics for
//! the shapes the suggestion proxy handles: Go's syntax rules (nesting
//! limit 10000), its string decoding (invalid UTF-8 and lone surrogates
//! become U+FFFD) and its encoder escaping (HTML-safe, U+2028/2029).

const MAX_DEPTH: usize = 10000;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool,
    Number,
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// json.Unmarshal's validity check plus decoding into a value tree.
pub fn parse(input: &[u8]) -> Option<Value> {
    let mut p = Parser { s: input, i: 0 };
    let v = p.value()?;
    p.ws();
    (p.i == input.len()).then_some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

enum Frame {
    Array(Vec<Value>),
    Object(Vec<(String, Value)>, String),
}

impl Parser<'_> {
    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.s.get(self.i) {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    // Iterative so that deeply nested (but legal for Go) input cannot
    // overflow the stack.
    fn value(&mut self) -> Option<Value> {
        let mut stack: Vec<Frame> = Vec::new();
        loop {
            self.ws();
            let mut v = match self.peek()? {
                b'[' => {
                    self.i += 1;
                    if stack.len() + 1 > MAX_DEPTH {
                        return None;
                    }
                    self.ws();
                    if self.eat(b']') {
                        Value::Array(Vec::new())
                    } else {
                        stack.push(Frame::Array(Vec::new()));
                        continue;
                    }
                }
                b'{' => {
                    self.i += 1;
                    if stack.len() + 1 > MAX_DEPTH {
                        return None;
                    }
                    self.ws();
                    if self.eat(b'}') {
                        Value::Object(Vec::new())
                    } else {
                        let key = self.key()?;
                        stack.push(Frame::Object(Vec::new(), key));
                        continue;
                    }
                }
                b'"' => Value::String(self.string()?),
                b't' => self.literal(b"true", Value::Bool)?,
                b'f' => self.literal(b"false", Value::Bool)?,
                b'n' => self.literal(b"null", Value::Null)?,
                b'-' | b'0'..=b'9' => self.number()?,
                _ => return None,
            };
            // Attach the completed value to its parent, closing containers
            // as their terminators arrive.
            loop {
                match stack.last_mut() {
                    None => return Some(v),
                    Some(Frame::Array(items)) => {
                        items.push(v);
                        self.ws();
                        if self.eat(b',') {
                            break;
                        }
                        if !self.eat(b']') {
                            return None;
                        }
                        let Some(Frame::Array(items)) = stack.pop() else {
                            unreachable!()
                        };
                        v = Value::Array(items);
                    }
                    Some(Frame::Object(fields, key)) => {
                        fields.push((std::mem::take(key), v));
                        self.ws();
                        if self.eat(b',') {
                            self.ws();
                            *key = self.key()?;
                            break;
                        }
                        if !self.eat(b'}') {
                            return None;
                        }
                        let Some(Frame::Object(fields, _)) = stack.pop() else {
                            unreachable!()
                        };
                        v = Value::Object(fields);
                    }
                }
            }
        }
    }

    fn key(&mut self) -> Option<String> {
        if self.peek()? != b'"' {
            return None;
        }
        let k = self.string()?;
        self.ws();
        self.eat(b':').then_some(k)
    }

    fn literal(&mut self, word: &[u8], v: Value) -> Option<Value> {
        if self.s[self.i..].starts_with(word) {
            self.i += word.len();
            Some(v)
        } else {
            None
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        self.i - start
    }

    fn number(&mut self) -> Option<Value> {
        self.eat(b'-');
        match self.peek()? {
            b'0' => self.i += 1,
            b'1'..=b'9' => {
                self.digits();
            }
            _ => return None,
        }
        if self.eat(b'.') && self.digits() == 0 {
            return None;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if self.digits() == 0 {
                return None;
            }
        }
        Some(Value::Number)
    }

    fn hex4(&self, at: usize) -> Option<u32> {
        let h = self.s.get(at..at + 4)?;
        let mut v = 0;
        for &c in h {
            v = v * 16 + (c as char).to_digit(16)?;
        }
        Some(v)
    }

    fn string(&mut self) -> Option<String> {
        self.i += 1; // opening quote
        let mut out = String::new();
        loop {
            let c = *self.s.get(self.i)?;
            match c {
                b'"' => {
                    self.i += 1;
                    return Some(out);
                }
                0..=0x1F => return None,
                b'\\' => {
                    let e = *self.s.get(self.i + 1)?;
                    self.i += 2;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let r = self.hex4(self.i)?;
                            self.i += 4;
                            if (0xD800..0xE000).contains(&r) {
                                // Pair with a following \uXXXX when it forms
                                // a valid surrogate pair, else U+FFFD.
                                if r < 0xDC00
                                    && self.s.get(self.i..self.i + 2) == Some(b"\\u")
                                    && let Some(r2) = self.hex4(self.i + 2)
                                    && (0xDC00..0xE000).contains(&r2)
                                {
                                    self.i += 6;
                                    let cp = 0x10000 + ((r - 0xD800) << 10) + (r2 - 0xDC00);
                                    out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                                } else {
                                    out.push('\u{FFFD}');
                                }
                            } else {
                                out.push(char::from_u32(r).unwrap_or('\u{FFFD}'));
                            }
                        }
                        _ => return None,
                    }
                }
                0x20..=0x7F => {
                    out.push(c as char);
                    self.i += 1;
                }
                _ => {
                    let (r, w) = crate::gocompat::decode_rune(&self.s[self.i..]);
                    out.push(r.unwrap_or('\u{FFFD}'));
                    self.i += w;
                }
            }
        }
    }
}

const HEX: &[u8; 16] = b"0123456789abcdef";

/// encoding/json string encoding with HTML escaping (the Encoder default).
pub fn write_string(dst: &mut Vec<u8>, s: &[u8]) {
    dst.push(b'"');
    let mut i = 0;
    while i < s.len() {
        let b = s[i];
        if b < 0x80 {
            match b {
                b'\\' | b'"' => dst.extend_from_slice(&[b'\\', b]),
                0x08 => dst.extend_from_slice(b"\\b"),
                0x0C => dst.extend_from_slice(b"\\f"),
                b'\n' => dst.extend_from_slice(b"\\n"),
                b'\r' => dst.extend_from_slice(b"\\r"),
                b'\t' => dst.extend_from_slice(b"\\t"),
                0..=0x1F | b'<' | b'>' | b'&' => {
                    dst.extend_from_slice(&[
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[usize::from(b >> 4)],
                        HEX[usize::from(b & 0xF)],
                    ]);
                }
                _ => dst.push(b),
            }
            i += 1;
            continue;
        }
        let (r, w) = crate::gocompat::decode_rune(&s[i..]);
        match r {
            None => dst.extend_from_slice(b"\\ufffd"),
            Some('\u{2028}') => dst.extend_from_slice(b"\\u2028"),
            Some('\u{2029}') => dst.extend_from_slice(b"\\u2029"),
            Some(_) => dst.extend_from_slice(&s[i..i + w]),
        }
        i += w;
    }
    dst.push(b'"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_go_valid_documents() {
        assert_eq!(
            parse(br#" ["a", ["b", null]] "#),
            Some(Value::Array(vec![
                Value::String("a".into()),
                Value::Array(vec![Value::String("b".into()), Value::Null])
            ]))
        );
        assert!(parse(b"1e999").is_some());
        assert!(parse(b"[1,]").is_none());
        assert!(parse(b"01").is_none());
        assert!(parse(b"[] x").is_none());
        assert!(parse(b"\"a\nb\"").is_none());
        assert!(parse(b"").is_none());
    }

    #[test]
    fn nesting_limit_matches_go() {
        let ok = format!("{}{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
        assert!(parse(ok.as_bytes()).is_some());
        let deep = format!("{}{}", "[".repeat(MAX_DEPTH + 1), "]".repeat(MAX_DEPTH + 1));
        assert!(parse(deep.as_bytes()).is_none());
    }

    #[test]
    fn string_decoding_replaces_invalid_sequences() {
        assert_eq!(
            parse(b"\"\\ud83d\\ude00\""),
            Some(Value::String("😀".into()))
        );
        assert_eq!(
            parse(b"\"\\ud83dx\""),
            Some(Value::String("\u{FFFD}x".into()))
        );
        assert_eq!(
            parse(b"\"\\ude00\""),
            Some(Value::String("\u{FFFD}".into()))
        );
        assert_eq!(
            parse(b"\"\xF0\x9F\x98a\""),
            Some(Value::String("\u{FFFD}\u{FFFD}\u{FFFD}a".into()))
        );
    }

    #[test]
    fn encoder_escapes_like_go() {
        let mut out = Vec::new();
        write_string(&mut out, "<a&b>\u{2028}\x01\x08\x7f\"\\".as_bytes());
        assert_eq!(
            out,
            br#""\u003ca\u0026b\u003e\u2028\u0001\b"#
                .iter()
                .copied()
                .chain(*b"\x7f\\\"\\\\\"")
                .collect::<Vec<_>>()
        );
        let mut out = Vec::new();
        write_string(&mut out, b"g \xff");
        assert_eq!(out, br#""g \ufffd""#);
    }
}
