//! Anthropic API keys blanked out of what Relay keeps and sends on. Relay never holds a key (the
//! key proxy does, docs/KEY-PROXY.md), but one someone pastes or prints would otherwise land in the terminal ring,
//! the stored scrollback, the phone's attach stream, a transcript reply or an audit row. Every key
//! starts `sk-ant-`; what follows it is overwritten with `*` byte for byte, so a terminal's columns
//! and a JSON document's shape are unchanged. A best effort: a key split by a cursor move or a
//! line wrap is blanked only up to the split.

use std::borrow::Cow;
use std::sync::LazyLock;

const PREFIX: &[u8] = b"sk-ant-";
static FINDER: LazyLock<memchr::memmem::Finder<'static>> = LazyLock::new(|| memchr::memmem::Finder::new(PREFIX));

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
    /// Every byte of a terminal's output passes here, so the walk byte by byte runs only where a
    /// key is open or a prefix may be: across a read's start and end, and after each prefix a
    /// SIMD search finds. A byte-at-a-time walk of every read cost 15 M instructions per MiB.
    pub fn redact(&mut self, bytes: &mut [u8]) {
        let mut i = self.walk(bytes, 0);
        while i < bytes.len() {
            // Most reads hold no `s` at all, and a byte search is cheaper to start than a
            // substring search: a read of short lines is often one line.
            let Some(s) = memchr::memchr(PREFIX[0], &bytes[i..]) else {
                return;
            };
            i += s;
            match FINDER.find(&bytes[i..]) {
                Some(at) => {
                    self.secret = true;
                    i = self.walk(bytes, i + at + PREFIX.len());
                }
                None => {
                    // No whole prefix left, but the read may end in the start of one.
                    let tail = bytes.len().saturating_sub(PREFIX.len() - 1).max(i);
                    for b in &mut bytes[tail..] {
                        self.step(b);
                    }
                    return;
                }
            }
        }
    }

    /// Steps from `from` while a key or a partial prefix is open; the index where neither is.
    fn walk(&mut self, bytes: &mut [u8], mut from: usize) -> usize {
        while from < bytes.len() && (self.secret || self.matched > 0) {
            self.step(&mut bytes[from]);
            from += 1;
        }
        from
    }

    fn step(&mut self, b: &mut u8) {
        if self.secret {
            if key_byte(*b) {
                *b = b'*';
                return;
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

    /// The walk byte by byte that `redact` skips ahead of, as the reference.
    fn every_byte(r: &mut Redactor, bytes: &mut [u8]) {
        for b in bytes.iter_mut() {
            r.step(b);
        }
    }

    #[test]
    fn the_search_blanks_what_a_walk_of_every_byte_does() {
        let streams: [&[u8]; 6] = [
            b"sk-sk-ant-abc sk-ant-\r\nssk-ant-x_y-z.",
            b"sk-ant-sk-ant-KEY1 sk-ant-",
            b"plain text with no key and some s, sk, sk-an, sk-ant",
            b"sk-ant-AAAA\x1b[0msk-ant-BBBB\nsk-an",
            b"ssssk-ant-ssss-k-ant-",
            b"sk-ant-",
        ];
        for stream in streams {
            for split in 0..=stream.len() {
                for second in split..=stream.len() {
                    let mut fast = stream.to_vec();
                    let mut slow = stream.to_vec();
                    let (mut f, mut s) = (Redactor::default(), Redactor::default());
                    for (from, to) in [(0, split), (split, second), (second, stream.len())] {
                        f.redact(&mut fast[from..to]);
                        every_byte(&mut s, &mut slow[from..to]);
                    }
                    assert_eq!(fast, slow, "{} split at {split} and {second}", String::from_utf8_lossy(stream));
                    assert_eq!((f.matched, f.secret), (s.matched, s.secret));
                }
            }
        }
    }

    /// Past the short streams above: a 4 KiB read, so memchr's and memmem's vector paths run, with
    /// keys past offset 64 beside high bytes and invalid UTF-8, split at a few places.
    #[test]
    fn a_long_read_with_high_bytes_blanks_what_a_walk_of_every_byte_does() {
        let mut stream = Vec::new();
        let mut n = 0u32;
        while stream.len() < 4096 {
            n += 1;
            stream.extend_from_slice(format!("line {n} ").as_bytes());
            stream.extend_from_slice(&[0xc3, 0xa9, 0xff, 0xfe, 0x80]);
            if n.is_multiple_of(7) {
                stream.extend_from_slice(b"\xe2sk-ant-api03-KEY");
                stream.extend_from_slice(n.to_string().as_bytes());
                stream.push(0xf0);
            }
            if n.is_multiple_of(11) {
                stream.extend_from_slice(b"ssk-an\x9fsk-");
            }
            stream.push(b'\n');
        }
        let keys = stream.windows(PREFIX.len()).filter(|w| *w == PREFIX).count();
        assert!(keys > 20 && stream.windows(PREFIX.len()).position(|w| w == PREFIX).unwrap() > 64);
        let len = stream.len();
        for splits in [vec![len], vec![64, len], vec![1, 2000, 2001, len], vec![99, 100, 101, 3000, len], (1..=len).step_by(97).chain([len]).collect()] {
            let mut fast = stream.clone();
            let mut slow = stream.clone();
            let (mut f, mut s) = (Redactor::default(), Redactor::default());
            let mut from = 0;
            for to in splits {
                f.redact(&mut fast[from..to]);
                every_byte(&mut s, &mut slow[from..to]);
                from = to;
            }
            assert_eq!(fast, slow);
            assert_eq!(fast.windows(PREFIX.len()).filter(|w| *w == PREFIX).count(), keys);
            assert!(!fast.windows(3).any(|w| w == b"KEY"), "every key blanked");
        }
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
