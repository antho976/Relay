//! SVG path data (`M3 6h18v12H3z`) as an icon node's `icon_path` and a saved icon hold it: read
//! strictly, written back in one plain form every painter reads (GSK's `Path::parse` on the
//! desktop, Compose's `PathParser` on the phone), and moved onto the 24-unit icon grid from
//! another viewBox.

/// Longest path data an icon holds, in characters, as written.
pub const MAX_PATH_DATA: usize = 8_000;
/// Most commands one holds.
pub const MAX_PATH_COMMANDS: usize = 2_000;

/// One command and its numbers, as many as one use of it takes.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub command: char,
    pub args: Vec<f64>,
}

/// How many numbers one use of `command` takes; `None` for a letter that is not a command.
fn arity(command: char) -> Option<usize> {
    Some(match command.to_ascii_lowercase() {
        'm' | 'l' | 't' => 2,
        'h' | 'v' => 1,
        'c' => 6,
        's' | 'q' => 4,
        'a' => 7,
        'z' => 0,
        _ => return None,
    })
}

/// Read path data: each command with its numbers, a repeated use of one spelt out (`M0 0 1 1` is
/// `M0 0 L1 1`), arc flags read as single digits (`a1 1 0 011 1`). Refused, with where, when
/// it does not start with a move, holds a letter that is not a command, a number short or one
/// that does not read, or passes [`MAX_PATH_DATA`] or [`MAX_PATH_COMMANDS`].
pub fn parse(d: &str) -> Result<Vec<Segment>, String> {
    if d.len() > MAX_PATH_DATA {
        return Err(format!("path data is {} characters; at most {MAX_PATH_DATA}", d.len()));
    }
    let bytes = d.as_bytes();
    let mut at = 0;
    let skip = |at: &mut usize| {
        while *at < bytes.len() && (bytes[*at].is_ascii_whitespace() || bytes[*at] == b',') {
            *at += 1;
        }
    };
    let mut out: Vec<Segment> = Vec::new();
    let mut current: Option<char> = None;
    loop {
        skip(&mut at);
        if at >= bytes.len() {
            break;
        }
        let c = bytes[at] as char;
        let command = if c.is_ascii_alphabetic() {
            at += 1;
            if arity(c).is_none() {
                return Err(format!("{c:?} at {} is not a path command (M L H V C S Q T A Z)", at - 1));
            }
            c
        } else {
            // A number after a command's last: that command again, a move's as a line.
            match current {
                Some(prev) if arity(prev) != Some(0) => match prev {
                    'M' => 'L',
                    'm' => 'l',
                    other => other,
                },
                _ => return Err(format!("a number at {at} follows no command")),
            }
        };
        if out.is_empty() && !matches!(command, 'M' | 'm') {
            return Err("path data starts with a move (M)".into());
        }
        let wanted = arity(command).unwrap_or(0);
        let mut args = Vec::with_capacity(wanted);
        for k in 0..wanted {
            skip(&mut at);
            let flag = command.eq_ignore_ascii_case(&'a') && (k == 3 || k == 4);
            let value = if flag {
                match bytes.get(at) {
                    Some(b'0') => 0.0,
                    Some(b'1') => 1.0,
                    _ => return Err(format!("an arc flag at {at} is 0 or 1")),
                }
            } else {
                let start = at;
                number_end(bytes, &mut at);
                let text = &d[start..at];
                match text.parse::<f64>() {
                    Ok(v) if v.is_finite() => v,
                    _ => return Err(format!("{command} needs {wanted} numbers; {:?} at {start} is not one", text)),
                }
            };
            if flag {
                at += 1;
            }
            args.push(value);
        }
        out.push(Segment { command, args });
        if out.len() > MAX_PATH_COMMANDS {
            return Err(format!("path data holds more than {MAX_PATH_COMMANDS} commands"));
        }
        current = Some(command);
    }
    if out.is_empty() {
        return Err("path data is empty".into());
    }
    Ok(out)
}

/// Move `at` past one number: a sign, digits, one point, an exponent.
fn number_end(bytes: &[u8], at: &mut usize) {
    if matches!(bytes.get(*at), Some(b'+' | b'-')) {
        *at += 1;
    }
    let mut point = false;
    while let Some(&b) = bytes.get(*at) {
        match b {
            b'0'..=b'9' => *at += 1,
            b'.' if !point => {
                point = true;
                *at += 1;
            }
            _ => break,
        }
    }
    if matches!(bytes.get(*at), Some(b'e' | b'E')) && bytes.get(*at + 1).is_some_and(|b| b.is_ascii_digit() || *b == b'-' || *b == b'+') {
        *at += 1;
        if matches!(bytes.get(*at), Some(b'+' | b'-')) {
            *at += 1;
        }
        while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
            *at += 1;
        }
    }
}

/// Path data in its plain form: each command once per use, numbers apart by spaces, to 3
/// decimals without trailing zeros (`M3 6 H21 V18 H3 Z`).
pub fn write(segments: &[Segment]) -> String {
    let mut out = String::new();
    for s in segments {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push(s.command);
        for (i, v) in s.args.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(&number(*v));
        }
    }
    out
}

fn number(v: f64) -> String {
    let v = (v * 1000.0).round() / 1000.0 + 0.0;
    let text = format!("{v:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".into() } else { text.to_string() }
}

/// `segments` drawn at `scale` and moved by `(dx, dy)`: absolute points scaled and moved,
/// relative ones and an arc's radii scaled, an arc's turn and flags kept.
pub fn transform(segments: &mut [Segment], scale: f64, dx: f64, dy: f64) {
    for s in segments.iter_mut() {
        let absolute = s.command.is_ascii_uppercase();
        let (mx, my) = if absolute { (dx, dy) } else { (0.0, 0.0) };
        match s.command.to_ascii_lowercase() {
            'h' => s.args[0] = s.args[0] * scale + mx,
            'v' => s.args[0] = s.args[0] * scale + my,
            'a' => {
                s.args[0] *= scale;
                s.args[1] *= scale;
                s.args[5] = s.args[5] * scale + mx;
                s.args[6] = s.args[6] * scale + my;
            }
            _ => {
                for pair in s.args.chunks_mut(2) {
                    pair[0] = pair[0] * scale + mx;
                    pair[1] = pair[1] * scale + my;
                }
            }
        }
    }
}

/// `d` read and written back in its plain form ([`parse`], [`write`]).
pub fn normalize(d: &str) -> Result<String, String> {
    parse(d).map(|segments| write(&segments))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_command_and_writes_it_plain() {
        assert_eq!(normalize("M3 6h18v12H3z").unwrap(), "M3 6 h18 v12 H3 z");
        assert_eq!(normalize("M4 11a7 7 0 1 0 14 0a7 7 0 1 0-14 0M21 21l-5-5").unwrap(), "M4 11 a7 7 0 1 0 14 0 a7 7 0 1 0 -14 0 M21 21 l-5 -5");
        // Repeats spelt out, a move's as lines; flags run together; exponents; points that start a number.
        assert_eq!(normalize("M0,0 1,1 2,2").unwrap(), "M0 0 L1 1 L2 2");
        assert_eq!(normalize("m0 0 1 1").unwrap(), "m0 0 l1 1");
        assert_eq!(normalize("M0 0a1 1 0 011 1").unwrap(), "M0 0 a1 1 0 0 1 1 1");
        assert_eq!(normalize("M1e1 .5L-.5.5").unwrap(), "M10 0.5 L-0.5 0.5");
        assert_eq!(normalize("M0 0C1 2 3 4 5 6S7 8 9 10Q1 1 2 2T3 3").unwrap(), "M0 0 C1 2 3 4 5 6 S7 8 9 10 Q1 1 2 2 T3 3");
        assert_eq!(normalize("M 12.3456 -0.0001 Z").unwrap(), "M12.346 0 Z");
    }

    #[test]
    fn refuses_what_does_not_read() {
        for (d, why) in [
            ("", "empty"),
            ("L1 1", "starts with a move"),
            ("M0 0 X1 1", "not a path command"),
            ("M0 0 L1", "L needs 2 numbers"),
            ("M0 0 a1 1 0 2 1 1 1", "arc flag"),
            ("M0 0 Z 1", "follows no command"),
            ("M0 0 Lx 1", "not one"),
        ] {
            let error = parse(d).unwrap_err();
            assert!(error.contains(why), "{d:?}: {error}");
        }
        assert!(parse(&format!("M0 0{}", "h1".repeat(MAX_PATH_COMMANDS))).unwrap_err().contains("more than"));
        assert!(parse(&"M0 0 ".repeat(2000)).unwrap_err().contains("at most"));
    }

    #[test]
    fn moves_onto_another_grid() {
        let mut s = parse("M0 0 H256 V256 l-128 -128 A64 32 15 1 0 128 128 Z").unwrap();
        transform(&mut s, 24.0 / 256.0, 0.0, 0.0);
        assert_eq!(write(&s), "M0 0 H24 V24 l-12 -12 A6 3 15 1 0 12 12 Z");
        let mut s = parse("M10 10 h4").unwrap();
        transform(&mut s, 2.0, -20.0, 1.0);
        assert_eq!(write(&s), "M0 21 h8");
    }
}
