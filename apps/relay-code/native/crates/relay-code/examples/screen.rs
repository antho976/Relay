//! Print the screen a `session.scrollback` answer (JSON on stdin) draws: for checking `screen.rs`.
use std::io::Read;
fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    let (cols, rows) = (v["cols"].as_u64().unwrap_or(120) as u16, v["rows"].as_u64().unwrap_or(40) as u16);
    let mut p = vt100::Parser::new(rows, cols, 0);
    p.process(v["text"].as_str().unwrap_or_default().as_bytes());
    println!("{cols}x{rows}\n{}", p.screen().contents());
}
