use std::collections::BTreeMap;

use anyhow::{Result, anyhow, bail};

/// A value of a Lua data module.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Num(f64),
    Bool(bool),
    Nil,
    Table(Table),
}

/// A Lua table: positional items in order, named fields by key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    pub items: Vec<Value>,
    pub fields: BTreeMap<String, Value>,
}

impl Table {
    pub fn field(&self, key: &str) -> Option<&Value> {
        self.fields.get(key)
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.field(key)?.str()
    }

    pub fn num(&self, key: &str) -> Option<f64> {
        self.field(key)?.num()
    }

    pub fn int(&self, key: &str) -> Option<i64> {
        self.num(key).map(|n| n as i64)
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.field(key)?.bool()
    }

    pub fn table(&self, key: &str) -> Option<&Table> {
        self.field(key)?.table()
    }
}

impl Value {
    pub fn str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn num(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn table(&self) -> Option<&Table> {
        match self {
            Value::Table(t) => Some(t),
            _ => None,
        }
    }
}

/// Read the table literal assigned to `name`, ignoring the rest of the module. A wiki data
/// module is Lua, not JSON: it holds several assignments, comments and a `return`, and only
/// one of them is the data.
pub fn table_of(src: &str, name: &str) -> Result<Table> {
    let start = assignment(src, name)
        .ok_or_else(|| anyhow!("no table assigned to '{name}' in this module"))?;
    let mut p = Parser {
        src: src.as_bytes(),
        at: start,
    };
    p.table()
}

/// Read the table a module returns directly, for the ones that skip the named local and
/// write `return { … }`.
pub fn returned(src: &str) -> Result<Table> {
    let start = assignment_after(src, "return")
        .ok_or_else(|| anyhow!("this module returns no table of its own"))?;
    let mut p = Parser {
        src: src.as_bytes(),
        at: start,
    };
    p.table()
}

/// Byte offset of the `{` following the keyword, with nothing but space between them.
fn assignment_after(src: &str, keyword: &str) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut from = 0;
    while let Some(hit) = src[from..].find(keyword) {
        let at = from + hit;
        from = at + keyword.len();
        let before_ok = at == 0 || !is_word(bytes[at - 1]);
        if !before_ok || from >= bytes.len() || is_word(bytes[from]) {
            continue;
        }
        let mut p = Parser { src: bytes, at: from };
        p.space();
        if p.peek() == Some(b'{') {
            return Some(p.at);
        }
    }
    None
}

/// Byte offset of the `{` opening the table assigned to `name`.
fn assignment(src: &str, name: &str) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut from = 0;
    while let Some(hit) = src[from..].find(name) {
        let at = from + hit;
        from = at + name.len();
        let before_ok = at == 0 || !is_word(bytes[at - 1]);
        if !before_ok || from >= bytes.len() || is_word(bytes[from]) {
            continue;
        }
        // The name may be written plain, quoted, or as a bracketed key: `D`, `"D"`, `["D"]`.
        let mut p = Parser { src: bytes, at: from };
        if p.peek() == Some(b'"') || p.peek() == Some(b'\'') {
            p.at += 1;
        }
        p.space();
        let _ = p.take(b']');
        p.space();
        if p.take(b'=') {
            p.space();
            if p.peek() == Some(b'{') {
                return Some(p.at);
            }
        }
    }
    None
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

struct Parser<'a> {
    src: &'a [u8],
    at: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.src.get(self.at).copied()
    }

    fn take(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.at += 1;
            return true;
        }
        false
    }

    fn starts(&self, s: &str) -> bool {
        self.src[self.at..].starts_with(s.as_bytes())
    }

    /// Skip whitespace and comments, in any mix.
    fn space(&mut self) {
        loop {
            while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
                self.at += 1;
            }
            if self.starts("--[[") {
                self.at += 4;
                while self.at < self.src.len() && !self.starts("]]") {
                    self.at += 1;
                }
                self.at = (self.at + 2).min(self.src.len());
                continue;
            }
            if self.starts("--") {
                while self.peek().is_some_and(|b| b != b'\n') {
                    self.at += 1;
                }
                continue;
            }
            return;
        }
    }

    fn value(&mut self) -> Result<Value> {
        self.space();
        match self.peek() {
            Some(b'{') => Ok(Value::Table(self.table()?)),
            Some(b'"') | Some(b'\'') => Ok(Value::Str(self.quoted()?)),
            Some(b'[') if self.starts("[[") => Ok(Value::Str(self.long_string())),
            Some(b) if b.is_ascii_digit() || b == b'-' || b == b'.' => Ok(Value::Num(self.expr()?)),
            _ => self.word(),
        }
    }

    /// `true`, `false`, `nil` — anything else in a data module is not data.
    fn word(&mut self) -> Result<Value> {
        let from = self.at;
        while self.peek().is_some_and(is_word) {
            self.at += 1;
        }
        match &self.src[from..self.at] {
            b"true" => Ok(Value::Bool(true)),
            b"false" => Ok(Value::Bool(false)),
            b"nil" => Ok(Value::Nil),
            other if other.is_empty() => bail!("unexpected character at byte {}", self.at),
            other => bail!(
                "'{}' at byte {from} is code, not data",
                String::from_utf8_lossy(other)
            ),
        }
    }

    fn table(&mut self) -> Result<Table> {
        if !self.take(b'{') {
            bail!("expected a table at byte {}", self.at);
        }
        let mut out = Table::default();
        loop {
            self.space();
            if self.take(b'}') {
                return Ok(out);
            }
            if self.at >= self.src.len() {
                bail!("table left open");
            }
            match self.key()? {
                Some(key) => {
                    let value = self.value()?;
                    out.fields.insert(key, value);
                }
                None => {
                    let value = self.value()?;
                    out.items.push(value);
                }
            }
            self.space();
            // A separator is optional before `}`; Lua accepts both `,` and `;`.
            let _ = self.take(b',') || self.take(b';');
        }
    }

    /// A field name, when the next token is one: `key =`, `["key"] =` or `[7] =`.
    fn key(&mut self) -> Result<Option<String>> {
        self.space();
        let mark = self.at;
        if self.take(b'[') && !self.starts("[") {
            self.space();
            let key = match self.peek() {
                Some(b'"') | Some(b'\'') => self.quoted()?,
                _ => {
                    let n = self.num()?;
                    format!("{n}")
                }
            };
            self.space();
            if self.take(b']') {
                self.space();
                if self.take(b'=') {
                    return Ok(Some(key));
                }
            }
            self.at = mark;
            return Ok(None);
        }
        self.at = mark;

        let from = self.at;
        while self.peek().is_some_and(is_word) {
            self.at += 1;
        }
        if self.at == from {
            return Ok(None);
        }
        let word = String::from_utf8_lossy(&self.src[from..self.at]).into_owned();
        self.space();
        if self.take(b'=') && self.peek() != Some(b'=') {
            return Ok(Some(word));
        }
        self.at = mark;
        Ok(None)
    }

    fn quoted(&mut self) -> Result<String> {
        let quote = self.peek().ok_or_else(|| anyhow!("string left open"))?;
        self.at += 1;
        let mut out = Vec::new();
        while let Some(b) = self.peek() {
            self.at += 1;
            match b {
                b'\\' => {
                    let esc = self.peek().ok_or_else(|| anyhow!("escape left open"))?;
                    self.at += 1;
                    out.push(match esc {
                        b'n' => b'\n',
                        b't' => b'\t',
                        b'r' => b'\r',
                        other => other,
                    });
                }
                b if b == quote => return Ok(String::from_utf8_lossy(&out).into_owned()),
                b => out.push(b),
            }
        }
        bail!("string left open")
    }

    /// A `[[ … ]]` long string.
    fn long_string(&mut self) -> String {
        self.at += 2;
        let from = self.at;
        while self.at < self.src.len() && !self.starts("]]") {
            self.at += 1;
        }
        let text = String::from_utf8_lossy(&self.src[from..self.at]).into_owned();
        self.at = (self.at + 2).min(self.src.len());
        text
    }

    /// A number, possibly written as arithmetic: the modules spell a stack size `5 * 80` to
    /// show what it is made of. Multiplication binds tighter than addition, as in Lua.
    fn expr(&mut self) -> Result<f64> {
        let mut value = self.term()?;
        loop {
            self.space();
            match self.peek() {
                Some(b'+') => {
                    self.at += 1;
                    self.space();
                    value += self.term()?;
                }
                // `--` opens a comment, so a lone `-` is the only subtraction.
                Some(b'-') if self.src.get(self.at + 1) != Some(&b'-') => {
                    self.at += 1;
                    self.space();
                    value -= self.term()?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn term(&mut self) -> Result<f64> {
        let mut value = self.num()?;
        loop {
            self.space();
            match self.peek() {
                Some(b'*') => {
                    self.at += 1;
                    self.space();
                    value *= self.num()?;
                }
                Some(b'/') => {
                    self.at += 1;
                    self.space();
                    value /= self.num()?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn num(&mut self) -> Result<f64> {
        let from = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_digit() || b == b'.' || b == b'e' || b == b'E')
        {
            self.at += 1;
        }
        let text = String::from_utf8_lossy(&self.src[from..self.at]);
        text.parse()
            .map_err(|_| anyhow!("'{text}' at byte {from} is not a number"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fields_items_and_nesting() {
        let src = r#"
local Data = {
    ["Name"] = "Apollodorus",
    Planet = 'Mercury',
    Index = 2,
    Chance = 0.3872,
    IsEndless = false,
    Nodes = { "a", "b" },
}
return Data
"#;
        let t = table_of(src, "Data").unwrap();
        assert_eq!(t.str("Name"), Some("Apollodorus"));
        assert_eq!(t.str("Planet"), Some("Mercury"));
        assert_eq!(t.int("Index"), Some(2));
        assert_eq!(t.num("Chance"), Some(0.3872));
        assert_eq!(t.bool("IsEndless"), Some(false));
        let nodes = t.table("Nodes").unwrap();
        assert_eq!(nodes.items.len(), 2);
        assert_eq!(nodes.items[0].str(), Some("a"));
    }

    #[test]
    fn picks_the_named_assignment_out_of_several() {
        let src = "local Other = { A = 1 }\nlocal Want = { A = 2 }\nreturn Want";
        assert_eq!(table_of(src, "Want").unwrap().int("A"), Some(2));
        assert_eq!(table_of(src, "Other").unwrap().int("A"), Some(1));
    }

    #[test]
    fn skips_comments_anywhere() {
        let src = "local D = {\n -- a line comment\n A = 1, --[[ inline ]] B = 2,\n}";
        let t = table_of(src, "D").unwrap();
        assert_eq!((t.int("A"), t.int("B")), (Some(1), Some(2)));
    }

    #[test]
    fn survives_ragged_whitespace_and_trailing_separators() {
        let src = " \t[\"D\"] \t= \t{\n\t\t{ Name = \"x\" };\n\t\t{ Name = \"y\" },\n\t}";
        let t = table_of(src, "D").unwrap();
        assert_eq!(t.items.len(), 2);
        assert_eq!(t.items[1].table().unwrap().str("Name"), Some("y"));
    }

    #[test]
    fn reads_escapes_and_long_strings() {
        let src = r#"local D = { Note = "*[[Plains]], [[Earth]]\n*talk to [[Konzu]]", Raw = [[as is]] }"#;
        let t = table_of(src, "D").unwrap();
        assert!(t.str("Note").unwrap().contains('\n'));
        assert_eq!(t.str("Raw"), Some("as is"));
    }

    #[test]
    fn reads_a_stack_size_written_as_arithmetic() {
        let src = r#"local D = { Rows = { { "Endo", "Resource", 18.97, 5 * 80 }, { "x", "y", 1, 2 + 3 * 4 } } }"#;
        let rows = table_of(src, "D").unwrap();
        let rows = rows.table("Rows").unwrap();
        assert_eq!(rows.items[0].table().unwrap().items[3].num(), Some(400.0));
        assert_eq!(rows.items[1].table().unwrap().items[3].num(), Some(14.0));
    }

    #[test]
    fn a_comment_after_a_number_is_not_subtraction() {
        let src = "local D = { A = 5, -- five\n B = 6 }";
        let t = table_of(src, "D").unwrap();
        assert_eq!((t.num("A"), t.num("B")), (Some(5.0), Some(6.0)));
    }

    #[test]
    fn a_missing_assignment_is_an_error_not_an_empty_table() {
        assert!(table_of("local D = { A = 1 }", "Missing").is_err());
    }

    #[test]
    fn code_where_data_was_expected_fails_loudly() {
        let err = table_of("local D = { A = mw.loadData('x') }", "D").unwrap_err();
        assert!(err.to_string().contains("is code, not data"), "{err}");
    }
}
