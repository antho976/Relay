//! Anthropic API keys blanked out of what Relay keeps and sends on. Relay never holds a key (the
//! key proxy does, docs/KEY-PROXY.md), but one someone pastes or prints would otherwise land in the terminal ring,
//! the stored scrollback, the phone's attach stream, a transcript reply or an audit row. Every key
//! starts `sk-ant-`; what follows it is overwritten with `*` byte for byte, so a terminal's columns
//! and a JSON document's shape are unchanged. A best effort: a key split by a cursor move or a
//! line wrap is blanked only up to the split.

use std::borrow::Cow;

const PREFIX: &[u8] = b"sk-ant-";

fn key_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

/// Blanks keys in a stream of chunks: a key, or its prefix, may straddle two reads.
#[derive(Debug, Default)]
pub struct Redactor {
    matched: usize,
    secret: bool,
}

impl Redactor {
    pub fn redact(&mut self, bytes: &mut [u8]) {
        for b in bytes.iter_mut() {
            if self.secret {
                if key_byte(*b) {
                    *b = b'*';
                    continue;
                }
                self.secret = false;
            }
            if *b == PREFIX[self.matched] {
                self.matched += 1;
                if self.matched == PREFIX.len() {
                    self.matched = 0;
                    self.secret = true;
                }
            } else {
                // The prefix repeats no part of itself but its first byte.
                self.matched = usize::from(*b == PREFIX[0]);
            }
        }
    }
}

/// One whole text. Key bytes are ASCII, so the result is valid UTF-8 whenever `text` was.
pub fn text(text: &str) -> Cow<'_, str> {
    if !text.contains("sk-ant-") {
        return Cow::Borrowed(text);
    }
    let mut bytes = text.as_bytes().to_vec();
    Redactor::default().redact(&mut bytes);
    Cow::Owned(String::from_utf8(bytes).expect("only ASCII bytes are replaced"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_blanked_to_the_same_length_and_nothing_else_is() {
        let key = "sk-ant-api03-AbC_d-1234";
        let line = format!("export K=\"{key}\"; echo ssk-ant done");
        let out = text(&line);
        assert_eq!(out.len(), line.len());
        assert_eq!(out, format!("export K=\"sk-ant-{}\"; echo ssk-ant done", "*".repeat(key.len() - 7)));
        assert!(matches!(text("no secrets here, sk-an and sk-"), Cow::Borrowed(_)));
        // Still a JSON document.
        let json = serde_json::json!({"command": format!("curl -H 'x-api-key: {key}'")}).to_string();
        let parsed: serde_json::Value = serde_json::from_str(&text(&json)).unwrap();
        assert!(!parsed["command"].as_str().unwrap().contains("api03"));
    }

    #[test]
    fn a_key_split_across_reads_is_blanked_whole() {
        let stream = b"key: sk-ant-api03-SECRETSECRET\r\nnext";
        for split in 0..stream.len() {
            let (mut a, mut b) = (stream[..split].to_vec(), stream[split..].to_vec());
            let mut r = Redactor::default();
            r.redact(&mut a);
            r.redact(&mut b);
            let joined = [a, b].concat();
            let expected = format!("key: sk-ant-{}\r\nnext", "*".repeat("api03-SECRETSECRET".len()));
            assert_eq!(joined, expected.as_bytes(), "split at {split}");
        }
    }
}
