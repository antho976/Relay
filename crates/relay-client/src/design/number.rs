//! Numbers as the canvas shows and reads them (client-text): the inspector's fields, formatted and
//! parsed.
//!
//! [`fmt`] writes what the size pill and the number fields show, and [`eval`] reads what a person
//! types or pastes into a field, from any keyboard: what it shows reads back as itself.

/// A number as the fields and the size pill show it: up to two decimals, as typed (13.5 stays
/// 13.5, 1.25 stays 1.25), a whole number bare, never "-0".
pub fn fmt(v: f64) -> String {
    // Up to two decimals, as typed: 13.5 stays 13.5 and 1.25 stays 1.25.
    let text = format!("{:.2}", (v * 100.0).round() / 100.0);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".into() } else { text.into() }
}

/// A number, or one sum on two (`120+16`, `200/2`, `3*4`, `10-4`), as design tools accept; a unit
/// typed after a number (`24px`, `50%`, `45°`, in any case) is let be. Typed on any keyboard or
/// pasted from a document: fullwidth digits and signs, the minus, times and division signs, a
/// decimal comma (`1,5`), and commas, dots or narrow spaces between thousands (`1,440`,
/// `1.440,5`). `None` for anything else, a division by zero, and what is not finite.
pub fn eval(text: &str) -> Option<f64> {
    let t = normalized(text);
    let t = t.trim();
    if let Some(v) = number(t) {
        return Some(v);
    }
    for op in ['+', '*', '/', '-'] {
        if let Some((i, _)) = t.char_indices().skip(1).find(|(_, c)| *c == op) {
            let (a, b) = (number(&t[..i])?, number(&t[i + 1..])?);
            let v = match op {
                '+' => a + b,
                '-' => a - b,
                '*' => a * b,
                _ if b != 0.0 => a / b,
                _ => return None,
            };
            return v.is_finite().then_some(v);
        }
    }
    None
}

/// One finite number, with a unit after it let be.
fn number(text: &str) -> Option<f64> {
    let mut t = text.trim();
    while let Some(rest) = ["px", "%", "°"].iter().find_map(|unit| {
        let cut = t.len().saturating_sub(unit.len());
        t.get(cut..).filter(|end| end.eq_ignore_ascii_case(unit)).map(|_| &t[..cut])
    }) {
        t = rest.trim_end();
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// `text` in the ASCII `eval` reads: fullwidth forms as their ASCII letters, the minus, dash,
/// times and division signs as `-`, `*` and `/`, the spaces that only group digits left out, and
/// each figure's separators as `str::parse` wants them ([`figure`]).
fn normalized(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\u{FF01}'..='\u{FF5E}' => out.extend(char::from_u32(c as u32 - 0xFEE0)),
            '\u{3000}' => out.push(' '),
            '\u{2212}' | '\u{2012}' | '\u{2013}' | '\u{FE63}' => out.push('-'),
            '\u{00D7}' => out.push('*'),
            '\u{00F7}' | '\u{2215}' => out.push('/'),
            '\u{00A0}' | '\u{2007}' | '\u{2009}' | '\u{202F}' => {}
            c => out.push(c),
        }
    }
    let part = |c: char| c.is_ascii_digit() || c == '.' || c == ',';
    let mut figures = String::with_capacity(out.len());
    let mut rest = out.as_str();
    while let Some(start) = rest.find(part) {
        let len = rest[start..].find(|c: char| !part(c)).unwrap_or(rest.len() - start);
        figures.push_str(&rest[..start]);
        figures.push_str(&figure(&rest[start..start + len]));
        rest = &rest[start + len..];
    }
    figures.push_str(rest);
    figures
}

/// A run of digits, dots and commas with a dot for its decimals and nothing between its
/// thousands. With both marks the last one is the decimal point (`1,440.5`, `1.440,5`); one kind
/// repeated between groups of three digits groups thousands (`1,234,567`, `1.234.567`), and so
/// does a single comma before three digits (`1,440`); any other single comma is a decimal comma
/// (`1,5`). What fits none of these is left for the parse to refuse.
fn figure(run: &str) -> String {
    let thousands = |mark: char| {
        let mut groups = run.split(mark);
        let lead = groups.next().unwrap_or("");
        (1..=3).contains(&lead.len()) && !lead.starts_with('0') && groups.all(|g| g.len() == 3)
    };
    match (run.rfind('.'), run.rfind(',')) {
        (Some(dot), Some(comma)) if comma > dot => run.replace('.', "").replace(',', "."),
        (Some(_), Some(_)) => run.replace(',', ""),
        (None, Some(_)) if thousands(',') => run.replace(',', ""),
        (None, Some(_)) if run.matches(',').count() == 1 => run.replace(',', "."),
        (Some(_), None) if run.matches('.').count() > 1 && thousands('.') => run.replace('.', ""),
        _ => run.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_take_arithmetic() {
        assert_eq!(eval("12"), Some(12.0));
        assert_eq!(eval("120+16"), Some(136.0));
        assert_eq!(eval("200/2"), Some(100.0));
        assert_eq!(eval("-8"), Some(-8.0));
        assert_eq!(eval("10-4"), Some(6.0));
        assert_eq!(eval("-2-3"), Some(-5.0));
        assert_eq!(eval("5*-2"), Some(-10.0));
        assert_eq!(eval("2-1e-1"), Some(1.9));
        assert_eq!(eval(" 3 * 4 "), Some(12.0));
        assert_eq!(eval("24px"), Some(24.0));
        assert_eq!(eval("50%"), Some(50.0));
        assert_eq!(eval("45°"), Some(45.0));
        assert_eq!(eval("1/0"), None);
        assert_eq!(eval("abc"), None);
        assert_eq!(eval(""), None);
        assert_eq!(eval("inf"), None);
        assert_eq!(eval("NaN"), None);
        assert_eq!(eval("1e400"), None);
    }

    #[test]
    fn eval_accepts_locale_and_unicode_input() {
        // A decimal comma, and a comma between thousands.
        assert_eq!(eval("1,5"), Some(1.5));
        assert_eq!(eval("1,25"), Some(1.25));
        assert_eq!(eval("1,440"), Some(1440.0));
        assert_eq!(eval("1,234,567"), Some(1_234_567.0));
        assert_eq!(eval("1,440.5"), Some(1440.5));
        assert_eq!(eval("1.440,5"), Some(1440.5));
        assert_eq!(eval("1.234.567"), Some(1_234_567.0));
        assert_eq!(eval("1.440"), Some(1.44));
        assert_eq!(eval("0,500"), Some(0.5));
        assert_eq!(eval(",5"), Some(0.5));
        assert_eq!(eval("1,5e3"), Some(1500.0));
        assert_eq!(eval("12,5+1,440"), Some(1452.5));
        assert_eq!(eval("1\u{202F}440"), Some(1440.0));
        // Fullwidth digits and operators, as an input method types them.
        assert_eq!(eval("１２０＋１６"), Some(136.0));
        assert_eq!(eval("２４ｐｘ"), Some(24.0));
        // The minus, times and division signs pasted from documents.
        assert_eq!(eval("10−4"), Some(6.0));
        assert_eq!(eval("−8"), Some(-8.0));
        assert_eq!(eval("10–4"), Some(6.0));
        assert_eq!(eval("3×4"), Some(12.0));
        assert_eq!(eval("12÷4"), Some(3.0));
        // A unit in any case, and on each side of a sum.
        assert_eq!(eval("24PX"), Some(24.0));
        assert_eq!(eval("120px + 16px"), Some(136.0));
        assert_eq!(eval("1,5,3"), None);
    }

    #[test]
    fn fmt_writes_what_the_fields_show() {
        // As board::fmt, the size pill's: whole numbers bare, up to two decimals, never "-0".
        for (v, text) in [
            (100.0, "100"),
            (13.5, "13.5"),
            (1.25, "1.25"),
            (1.005, "1"),
            (0.333, "0.33"),
            (-0.0, "0"),
            (-0.001, "0"),
            (-12.4, "-12.4"),
            (1440.0, "1440"),
        ] {
            assert_eq!(fmt(v), text, "{v}");
        }
        // What a field shows reads back as itself.
        for v in [0.0, 1.5, -7.25, 1440.0, 99.99] {
            assert_eq!(eval(&fmt(v)), Some(v));
        }
    }
}
