//! Numbers as the native client writes them, one rule each, so the spaces' pages and panels
//! agree: grouped counts, signed figures and percents, byte sizes and plurals. Money itself is
//! `relay_money::money::MoneyFormatter`'s (Tally's rules, locale and all); these are the words
//! around it.

/// The minus sign every figure is written with, never the hyphen.
pub const MINUS: char = '\u{2212}';

/// Whether `shown` is a figure other than zero: it has a digit that is not 0. "$0", "0.0%" and
/// "−$0" are zero; "$0.40" is not.
pub fn shows_nonzero(shown: &str) -> bool {
    shown.chars().any(|c| c.is_ascii_digit() && c != '0')
}

/// `shown`, a figure already written for `value`, with its sign decided by what it shows: a plus
/// on a gain, and nothing on a figure that rounded to zero ("+$0" and "−$0" read as "$0", given
/// as `zero`). A loss keeps the minus it was written with.
pub fn signed(shown: String, value: f64, zero: impl FnOnce() -> String) -> String {
    if !shows_nonzero(&shown) {
        zero()
    } else if value > 0.0 {
        format!("+{shown}")
    } else {
        shown
    }
}

/// 12,500 and −1,200.
pub fn group(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 3);
    if n < 0 {
        out.push(MINUS);
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A share in percent to one decimal: "+4.2%" and "−3.0%" when `signed`, "4.2%" else; a loss
/// always shows its minus. One that rounds to zero is "0.0%", without a sign either way.
pub fn percent(value: f64, signed: bool) -> String {
    let body = format!("{:.1}%", value.abs());
    if !shows_nonzero(&body) {
        "0.0%".to_string()
    } else if value < 0.0 {
        format!("{MINUS}{body}")
    } else if signed {
        format!("+{body}")
    } else {
        body
    }
}

/// [`percent`] of basis points: 1450 is "+14.5%".
pub fn percent_bps(bps: i64, signed: bool) -> String {
    percent(bps as f64 / 100.0, signed)
}

/// A signed share in whole percent: "+5%", "−12%", and "0%" for one that rounds to zero.
pub fn whole_percent(value: f64) -> String {
    match value.round() as i64 {
        0 => "0%".to_string(),
        r if r < 0 => format!("{MINUS}{}%", -r),
        r => format!("+{r}%"),
    }
}

/// A size in bytes as the desktop's own file manager writes it, in powers of 1000 (kB, MB, GB,
/// TB): one decimal below ten units ("1.5 MB"), whole above ("75 MB"), and never "0 kB" for a
/// file that is not empty.
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["kB", "MB", "GB", "TB"];
    if n < 1000 {
        return format!("{n} B");
    }
    let mut value = n as f64 / 1000.0;
    let mut unit = 0;
    // Step up once the figure would round to 1000 of this unit.
    while value >= 999.5 && unit + 1 < UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 || value >= 9.95 {
        format!("{} {}", (value.round() as u64).max(1), UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// [`bytes`] of mebibytes, the unit the engine reports memory and disk in (`rss_mb`, `disk_mb`).
pub fn mebibytes(mb: f64) -> String {
    bytes((mb.max(0.0) * 1_048_576.0).round() as u64)
}

/// "1 day", "3 days"; `many` for any count but one.
pub fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_figure_that_rounds_to_zero_has_no_sign() {
        let dollars = |cents: i64| format!("{}${}", if cents < 0 { "\u{2212}" } else { "" }, (cents.abs() + 50) / 100);
        let zero = || "$0".to_string();
        assert_eq!(signed(dollars(49), 0.49, zero), "$0", "not +$0");
        assert_eq!(signed(dollars(-49), -0.49, zero), "$0", "not −$0");
        assert_eq!(signed(dollars(150), 1.5, zero), "+$2");
        assert_eq!(signed(dollars(-150), -1.5, zero), "\u{2212}$2");
        assert_eq!(signed("$0.40".into(), 0.4, zero), "+$0.40");
    }

    #[test]
    fn groups_of_three_with_the_minus_sign() {
        assert_eq!(group(0), "0");
        assert_eq!(group(999), "999");
        assert_eq!(group(12_500), "12,500");
        assert_eq!(group(-1_200), "\u{2212}1,200");
        assert_eq!(group(1_234_567), "1,234,567");
        assert_eq!(group(i64::MIN), "\u{2212}9,223,372,036,854,775,808");
    }

    #[test]
    fn a_percent_is_signed_by_what_it_shows() {
        assert_eq!(percent(4.24, true), "+4.2%");
        assert_eq!(percent(-3.0, true), "\u{2212}3.0%");
        assert_eq!(percent(0.04, true), "0.0%", "1–4 bps is not +0.0%");
        assert_eq!(percent(-0.04, false), "0.0%");
        assert_eq!(percent(12.34, false), "12.3%");
        assert_eq!(percent(-12.34, false), "\u{2212}12.3%");
        assert_eq!(percent_bps(1450, true), "+14.5%");
        assert_eq!(percent_bps(3, true), "0.0%");
        assert_eq!(whole_percent(4.6), "+5%");
        assert_eq!(whole_percent(-12.2), "\u{2212}12%");
        assert_eq!(whole_percent(0.0), "0%");
        assert_eq!(whole_percent(-0.4), "0%");
    }

    #[test]
    fn sizes_are_in_powers_of_a_thousand() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(999), "999 B");
        assert_eq!(bytes(1_000), "1 kB");
        assert_eq!(bytes(1_499), "1 kB");
        assert_eq!(bytes(999_499), "999 kB");
        assert_eq!(bytes(999_500), "1.0 MB", "never 1000 kB");
        assert_eq!(bytes(1_500_000), "1.5 MB");
        assert_eq!(bytes(9_940_000), "9.9 MB");
        assert_eq!(bytes(9_960_000), "10 MB", "never 10.0 MB");
        assert_eq!(bytes(800_000_000), "800 MB");
        assert_eq!(bytes(2_400_000_000), "2.4 GB");
        assert_eq!(bytes(3_000_000_000_000_000), "3000 TB");
        assert_eq!(mebibytes(75.4), "79 MB");
        assert_eq!(mebibytes(-1.0), "0 B");
    }

    #[test]
    fn one_is_singular() {
        assert_eq!(plural(1, "day", "days"), "1 day");
        assert_eq!(plural(0, "day", "days"), "0 days");
        assert_eq!(plural(3, "entry", "entries"), "3 entries");
    }
}
