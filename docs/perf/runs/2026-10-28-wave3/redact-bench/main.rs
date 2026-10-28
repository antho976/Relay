#![allow(dead_code)]
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


mod old {
    const PREFIX: &[u8] = b"sk-ant-";
    fn key_byte(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'-' || b == b'_' }
    #[derive(Default)]
    pub struct Redactor { matched: usize, secret: bool }
    impl Redactor {
        pub fn redact(&mut self, bytes: &mut [u8]) {
            for b in bytes.iter_mut() {
                if self.secret { if key_byte(*b) { *b = b'*'; continue; } self.secret = false; }
                if *b == PREFIX[self.matched] { self.matched += 1; if self.matched == PREFIX.len() { self.matched = 0; self.secret = true; } }
                else { self.matched = usize::from(*b == PREFIX[0]); }
            }
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let text = std::fs::read(&a[1]).unwrap();
    let mut data: Vec<u8> = text.iter().cycle().take(1 << 20).copied().collect();
    let n: usize = a[3].parse().unwrap();
    let chunk: usize = a[4].parse().unwrap();
    if a[2] == "old" { let mut r = old::Redactor::default(); for _ in 0..n { for c in data.chunks_mut(chunk) { r.redact(c); } } }
    else { let mut r = Redactor::default(); for _ in 0..n { for c in data.chunks_mut(chunk) { r.redact(c); } } }
    std::hint::black_box(&data);
}
