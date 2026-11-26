//! Readers for Valve's KeyValues files: the text form (localconfig.vdf, libraryfolders.vdf) and
//! the binary form used inside appinfo.vdf, packageinfo.vdf and the achievement stats caches.

use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Obj(Vec<(String, Value)>),
    Str(String),
    Int(i64),
    Float(f32),
}

impl Value {
    /// Child by key, ignoring case (Steam is not consistent about it).
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(items) => items.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Follows a path of keys, e.g. `["Software", "Valve", "Steam"]`.
    pub fn path(&self, keys: &[&str]) -> Option<&Value> {
        keys.iter().try_fold(self, |v, k| v.get(k))
    }

    pub fn entries(&self) -> &[(String, Value)] {
        match self {
            Value::Obj(items) => items,
            _ => &[],
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Numbers are stored as text in some files and as integers in others.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            Value::Str(s) => s.trim().parse().ok(),
            Value::Obj(_) => None,
        }
    }
}

/// Parses the text format. Returns an object holding the top-level entries.
pub fn parse_text(text: &str) -> Result<Value> {
    let mut tokens = Tokenizer { chars: text.chars().peekable() };
    let items = parse_items(&mut tokens, true)?;
    Ok(Value::Obj(items))
}

enum Token {
    Str(String),
    Open,
    Close,
}

struct Tokenizer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl Tokenizer<'_> {
    fn next_token(&mut self) -> Result<Option<Token>> {
        loop {
            let Some(&c) = self.chars.peek() else { return Ok(None) };
            match c {
                c if c.is_whitespace() => {
                    self.chars.next();
                }
                '/' => {
                    // `// comment` to end of line.
                    while self.chars.next_if(|&c| c != '\n').is_some() {}
                }
                '{' => {
                    self.chars.next();
                    return Ok(Some(Token::Open));
                }
                '}' => {
                    self.chars.next();
                    return Ok(Some(Token::Close));
                }
                '"' => {
                    self.chars.next();
                    let mut s = String::new();
                    loop {
                        match self.chars.next() {
                            None => bail!("unterminated string"),
                            Some('"') => break,
                            Some('\\') => match self.chars.next() {
                                Some('n') => s.push('\n'),
                                Some('t') => s.push('\t'),
                                Some(other) => s.push(other),
                                None => bail!("unterminated string"),
                            },
                            Some(other) => s.push(other),
                        }
                    }
                    return Ok(Some(Token::Str(s)));
                }
                _ => {
                    // Unquoted word.
                    let mut s = String::new();
                    while let Some(c) = self.chars.next_if(|c| !c.is_whitespace() && !matches!(c, '{' | '}' | '"')) {
                        s.push(c);
                    }
                    return Ok(Some(Token::Str(s)));
                }
            }
        }
    }
}

fn parse_items(tokens: &mut Tokenizer, top: bool) -> Result<Vec<(String, Value)>> {
    let mut items = Vec::new();
    loop {
        let key = match tokens.next_token()? {
            None if top => return Ok(items),
            None => bail!("unexpected end of file"),
            Some(Token::Close) if !top => return Ok(items),
            Some(Token::Close) => bail!("unexpected closing brace"),
            Some(Token::Open) => bail!("expected a key"),
            Some(Token::Str(k)) => k,
        };
        let value = match tokens.next_token()? {
            Some(Token::Str(v)) => Value::Str(v),
            Some(Token::Open) => Value::Obj(parse_items(tokens, false)?),
            _ => bail!("expected a value for '{key}'"),
        };
        items.push((key, value));
    }
}

/// Reads binary KeyValues. `strings` is the key table used by appinfo.vdf v29; without it keys
/// are inline strings.
pub struct BinaryReader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
    pub strings: Option<&'a [String]>,
}

impl<'a> BinaryReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0, strings: None }
    }

    pub fn u8(&mut self) -> Result<u8> {
        let b = *self.data.get(self.pos).ok_or_else(|| anyhow::anyhow!("unexpected end of data"))?;
        self.pos += 1;
        Ok(b)
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len());
        let Some(end) = end else { bail!("unexpected end of data") };
        let out = &self.data[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into()?))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.bytes(4)?.try_into()?))
    }

    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into()?))
    }

    pub fn cstr(&mut self) -> Result<String> {
        let rest = &self.data[self.pos.min(self.data.len())..];
        let Some(len) = rest.iter().position(|&b| b == 0) else { bail!("unterminated string") };
        let s = String::from_utf8_lossy(&rest[..len]).into_owned();
        self.pos += len + 1;
        Ok(s)
    }

    fn key(&mut self) -> Result<String> {
        match self.strings {
            Some(table) => {
                let i = self.u32()? as usize;
                table.get(i).cloned().ok_or_else(|| anyhow::anyhow!("key index {i} out of range"))
            }
            None => self.cstr(),
        }
    }

    /// Reads entries until the end marker of the current object.
    pub fn object(&mut self) -> Result<Vec<(String, Value)>> {
        let mut items = Vec::new();
        loop {
            let kind = self.u8()?;
            if kind == 0x08 || kind == 0x0B {
                return Ok(items);
            }
            let key = self.key()?;
            let value = match kind {
                0x00 => Value::Obj(self.object()?),
                0x01 => Value::Str(self.cstr()?),
                0x02 | 0x04 | 0x06 => Value::Int(i64::from(self.i32()?)),
                0x03 => Value::Float(f32::from_le_bytes(self.bytes(4)?.try_into()?)),
                0x07 => Value::Int(self.u64()? as i64),
                0x0A => Value::Int(i64::from_le_bytes(self.bytes(8)?.try_into()?)),
                0x05 => {
                    // UTF-16 string, rare; read up to a double zero.
                    let mut units = Vec::new();
                    loop {
                        let u = u16::from_le_bytes(self.bytes(2)?.try_into()?);
                        if u == 0 {
                            break;
                        }
                        units.push(u);
                    }
                    Value::Str(String::from_utf16_lossy(&units))
                }
                other => bail!("unknown KeyValues type 0x{other:02x}"),
            };
            items.push((key, value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text() {
        let text = "\"Root\"\n{\n\t\"apps\"\n\t{\n\t\t\"620\" { \"Playtime\" \"95\" \"Name\" \"a \\\"b\\\"\" }\n\t}\n}\n// tail";
        let v = parse_text(text).unwrap();
        let app = v.path(&["root", "Apps", "620"]).unwrap();
        assert_eq!(app.get("playtime").and_then(Value::as_i64), Some(95));
        assert_eq!(app.get("name").and_then(Value::as_str), Some("a \"b\""));
    }

    #[test]
    fn parses_binary() {
        let mut data = vec![0x00];
        data.extend(b"root\0");
        data.push(0x01);
        data.extend(b"name\0Portal\0");
        data.push(0x02);
        data.extend(b"id\0");
        data.extend(400i32.to_le_bytes());
        data.extend([0x08, 0x08]);
        let v = Value::Obj(BinaryReader::new(&data).object().unwrap());
        assert_eq!(v.path(&["root", "name"]).and_then(Value::as_str), Some("Portal"));
        assert_eq!(v.path(&["root", "id"]).and_then(Value::as_i64), Some(400));
    }
}
