//! Front matter: a block of `key: value` lines between two `---` lines at the very top of a
//! note, as other Markdown apps write it.
//!
//! ```text
//! ---
//! sticker: 🌱
//! tags: [garden, plan]
//! ---
//! # Garden plan
//! ```
//!
//! Quire reads the top-level keys and keeps the block out of the preview. It reads a small part
//! of YAML: plain, quoted and empty values, lists as `[a, b]` or as `- a` lines, and indented
//! lines under a key, which stay as text. The block counts as front matter only when it closes
//! with `---` or `...` and each top-level line is a key, a comment or blank, so a note that
//! starts with a rule keeps it.

/// The value of one key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// A plain or quoted value, or the indented lines under a key, joined by `\n`.
    Text(String),
    /// `[a, b]`, or `- a` lines under the key.
    List(Vec<String>),
}

impl Value {
    /// The value as text, or `None` for a list.
    #[allow(dead_code)] // For the sticker and banner keys, which come later.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(t) => Some(t),
            Value::List(_) => None,
        }
    }
}

/// The front matter of a note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontMatter {
    /// The keys in the order the note gives them. A key given twice keeps its last value.
    pub fields: Vec<(String, Value)>,
    /// The number of lines the block takes, both `---` lines included.
    pub lines: usize,
    /// The byte offset in the note where the text after the block starts.
    #[allow(dead_code)] // For word counts and the like, which come later.
    pub end: usize,
}

impl FrontMatter {
    /// The value of `key`. Keys match in any case.
    #[allow(dead_code)] // For the sticker and banner keys, which come later.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.fields.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
    }
}

/// Reads the front matter at the top of `text`, or `None` when the note has none.
pub fn parse(text: &str) -> Option<FrontMatter> {
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    let bom = text.len() - body.len();
    let mut lines = body.split_inclusive('\n');
    let first = lines.next()?;
    if clean(first) != "---" {
        return None;
    }
    let mut at = bom + first.len();
    let mut inner: Vec<&str> = Vec::new();
    for (count, raw) in (2..).zip(lines) {
        at += raw.len();
        let ln = clean(raw);
        if ln == "---" || ln == "..." {
            return Some(FrontMatter { fields: fields(&inner)?, lines: count, end: at });
        }
        inner.push(ln);
    }
    None
}

/// A line without its line break and trailing spaces.
fn clean(raw: &str) -> &str {
    raw.trim_end_matches(['\n', '\r']).trim_end()
}

fn is_comment(ln: &str) -> bool {
    ln.trim_start().starts_with('#')
}

fn is_nested(ln: &str) -> bool {
    ln.starts_with([' ', '\t']) || ln.starts_with("- ") || ln == "-"
}

/// The keys of the block, or `None` when a top-level line is not a key.
fn fields(lines: &[&str]) -> Option<Vec<(String, Value)>> {
    let mut out: Vec<(String, Value)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let ln = lines[i];
        i += 1;
        if ln.trim().is_empty() || is_comment(ln) {
            continue;
        }
        if is_nested(ln) {
            return None;
        }
        let (key, rest) = split_key(ln)?;
        let value = if rest.is_empty() {
            let start = i;
            while i < lines.len() && (lines[i].trim().is_empty() || is_nested(lines[i]) || is_comment(lines[i])) {
                i += 1;
            }
            nested(&lines[start..i])
        } else {
            inline(rest)
        };
        out.retain(|(k, _)| k != &key);
        out.push((key, value));
    }
    Some(out)
}

/// `key: rest`, with the key unquoted. A key needs a `:` followed by a space or the line end.
fn split_key(ln: &str) -> Option<(String, &str)> {
    let colon = ln.char_indices().find(|&(i, c)| c == ':' && ln[i + 1..].chars().next().is_none_or(|n| n == ' ' || n == '\t'))?.0;
    let key = unquote(ln[..colon].trim());
    (!key.is_empty()).then(|| (key, ln[colon + 1..].trim()))
}

/// A value on the key's own line.
fn inline(v: &str) -> Value {
    if let Some(items) = v.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return Value::List(items.split(',').map(|s| unquote(s.trim())).filter(|s| !s.is_empty()).collect());
    }
    Value::Text(unquote(strip_comment(v)))
}

/// The lines under a key with nothing after its colon.
fn nested(lines: &[&str]) -> Value {
    let body: Vec<&str> = lines.iter().copied().filter(|l| !l.trim().is_empty() && !is_comment(l)).collect();
    if body.is_empty() {
        return Value::Text(String::new());
    }
    let items: Option<Vec<String>> = body
        .iter()
        .map(|l| {
            let t = l.trim_start();
            t.strip_prefix("- ").map(|s| unquote(strip_comment(s.trim()))).or_else(|| (t == "-").then(String::new))
        })
        .collect();
    if let Some(items) = items {
        return Value::List(items);
    }
    let indent = body.iter().map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    Value::Text(body.iter().map(|l| &l[indent.min(l.len() - l.trim_start().len())..]).collect::<Vec<_>>().join("\n"))
}

/// A value without a trailing ` # comment`. Quoted values keep a `#` inside the quotes.
fn strip_comment(v: &str) -> &str {
    if v.starts_with(['"', '\'']) {
        return v;
    }
    v.find(" #").map_or(v, |i| v[..i].trim_end())
}

/// A value without its surrounding quotes. Inside double quotes, `\"` and `\\` are unescaped.
fn unquote(v: &str) -> String {
    if v.len() >= 2 {
        if let Some(s) = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            return s.replace("\\\"", "\"").replace("\\\\", "\\");
        }
        if let Some(s) = v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
            return s.replace("''", "'");
        }
    }
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(v: &str) -> Value {
        Value::Text(v.into())
    }

    fn list(v: &[&str]) -> Value {
        Value::List(v.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn reads_the_keys() {
        let note = "---\nsticker: 🌱\nbanner: \"attachments/beds.png\"\ntitle: 'Garden ''plan'''\nempty:\ntags: [garden, \"plan\"]\n---\n# Garden plan\n";
        let fm = parse(note).unwrap();
        assert_eq!(fm.get("sticker"), Some(&text("🌱")));
        assert_eq!(fm.get("Banner").and_then(Value::as_text), Some("attachments/beds.png"));
        assert_eq!(fm.get("title"), Some(&text("Garden 'plan'")));
        assert_eq!(fm.get("empty"), Some(&text("")));
        assert_eq!(fm.get("tags"), Some(&list(&["garden", "plan"])));
        assert_eq!(fm.get("missing"), None);
        assert_eq!(fm.lines, 7);
        assert_eq!(&note[fm.end..], "# Garden plan\n");
    }

    #[test]
    fn reads_lists_nested_text_and_comments() {
        let note = "---\n# a comment\naliases:\n  - Plan\n  - \"Beds\" \ncover:\n  src: a.png\n  alt: Beds\nurl: https://example.com/a#b # where\nsticker: 🌱\nsticker: 🌻\n...\nText";
        let fm = parse(note).unwrap();
        assert_eq!(fm.get("aliases"), Some(&list(&["Plan", "Beds"])));
        assert_eq!(fm.get("cover"), Some(&text("src: a.png\nalt: Beds")));
        assert_eq!(fm.get("url"), Some(&text("https://example.com/a#b")));
        assert_eq!(fm.get("sticker"), Some(&text("🌻")), "the last value wins");
        assert_eq!(fm.fields.iter().filter(|(k, _)| k == "sticker").count(), 1);
        assert_eq!(&note[fm.end..], "Text");
    }

    #[test]
    fn handles_crlf_a_bom_and_an_empty_block() {
        let note = "\u{feff}---\r\nsticker: 🌱\r\n---\r\nBody";
        let fm = parse(note).unwrap();
        assert_eq!(fm.get("sticker"), Some(&text("🌱")));
        assert_eq!(&note[fm.end..], "Body");
        let empty = parse("---\n---\n").unwrap();
        assert!(empty.fields.is_empty());
        assert_eq!(empty.lines, 2);
        assert_eq!(empty.end, 8);
    }

    #[test]
    fn needs_a_real_block_at_the_top() {
        assert_eq!(parse("# Title\n---\na: b\n---\n"), None, "not at the top");
        assert_eq!(parse("---\na: b\n"), None, "never closes");
        assert_eq!(parse("---\nJust a paragraph between rules.\n---\n"), None, "not keys");
        assert_eq!(parse("---\n- a\n---\n"), None, "a list on its own");
        assert_eq!(parse("----\na: b\n----\n"), None);
        assert_eq!(parse(" ---\na: b\n---\n"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("---"), None);
    }

    #[test]
    fn a_colon_needs_a_space_after_it() {
        assert_eq!(parse("---\nhttp://example.com\n---\n"), None);
        assert_eq!(parse("---\ntime: 10:30\n---\n").unwrap().get("time"), Some(&text("10:30")));
    }
}
