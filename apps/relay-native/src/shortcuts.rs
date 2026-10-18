use gtk4::gdk::{self, prelude::*, Key, ModifierType};

pub const DEFAULTS: [(&str, &str, &str); 8] = [
    ("palette", "Command palette", "Ctrl+K"),
    ("agents", "Agents", "Ctrl+1"),
    ("code", "Files and Git", "Ctrl+2"),
    ("board", "Board", "Ctrl+3"),
    ("new_session", "New session", "Ctrl+N"),
    ("settings", "Settings", "Ctrl+,"),
    ("sidebar", "Toggle sidebar", "Ctrl+Shift+B"),
    ("space", "Switch between Dev and Threads", "Ctrl+`"),
];

fn parse(chord: &str) -> Option<(ModifierType, char)> {
    let mut mods = ModifierType::empty();
    let mut key = None;
    for part in chord.split('+').map(str::trim) {
        match part.to_lowercase().as_str() {
            "ctrl" => mods |= ModifierType::CONTROL_MASK,
            "meta" => mods |= ModifierType::META_MASK,
            "alt" => mods |= ModifierType::ALT_MASK,
            "shift" => mods |= ModifierType::SHIFT_MASK,
            s if s.chars().count() == 1 && key.is_none() => key = s.chars().next(),
            _ => return None,
        }
    }
    Some((mods, key?))
}

/// Shift with a digit or punctuation never fires: Shift turns the ',' of "Ctrl+Shift+," into '<'.
/// The shifted character itself ("Ctrl+<") is the chord for that key.
pub fn valid(chord: &str) -> bool {
    parse(chord).is_some_and(|(mods, key)| {
        mods.intersects(
            ModifierType::CONTROL_MASK | ModifierType::META_MASK | ModifierType::ALT_MASK,
        ) && (key.is_alphabetic() || !mods.contains(ModifierType::SHIFT_MASK))
    })
}

/// Ctrl+letter is a control character a terminal program reads (Ctrl+K kills to the end of
/// the line, Ctrl+N is next history), so a focused agent terminal keeps it.
pub fn terminal_owns(chord: &str) -> bool {
    parse(chord).is_some_and(|(mods, key)| mods == ModifierType::CONTROL_MASK && key.is_ascii_alphabetic())
}

/// `key`, or, when it is not Latin, what the same physical key gives unshifted in the first layout
/// that has a Latin character there. GTK's own accelerators fall back the same way, so Ctrl+S
/// still saves under a Cyrillic layout, where it types Cyrillic_yeru. Pass the key-pressed
/// signal's keyval and keycode, and match the result.
pub fn latin(key: Key, keycode: u32) -> Key {
    if key.to_unicode().is_none_or(|c| c.is_ascii()) {
        return key;
    }
    let Some(entries) = gdk::Display::default().and_then(|d| d.map_keycode(keycode)) else {
        return key;
    };
    entries
        .into_iter()
        .filter(|(k, latin)| k.level() == 0 && latin.to_unicode().is_some_and(|c| c.is_ascii_graphic()))
        .min_by_key(|(k, _)| k.group())
        .map_or(key, |(_, latin)| latin)
}

/// `key` is what `latin` returned for the press.
pub fn matches(key: Key, mods: ModifierType, chord: &str) -> bool {
    let mask = ModifierType::CONTROL_MASK
        | ModifierType::META_MASK
        | ModifierType::ALT_MASK
        | ModifierType::SHIFT_MASK;
    parse(chord).is_some_and(|(expected, letter)| {
        // Shift is spent on a shifted character, so "Ctrl+<" is Ctrl+Shift+, pressed.
        let mods = if letter.is_alphabetic() { mods & mask } else { mods & (mask - ModifierType::SHIFT_MASK) };
        mods == expected
            && key.to_unicode().and_then(|c| c.to_lowercase().next()) == Some(letter)
    })
}

#[test]
fn chords_preserve_modifiers_and_allow_disabling() {
    assert!(matches(Key::K, ModifierType::CONTROL_MASK, " ctrl + k "));
    assert!(!matches(
        Key::K,
        ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK,
        "Ctrl+K"
    ));
    assert!(matches(Key::comma, ModifierType::CONTROL_MASK, "Ctrl+,"));
    assert!(matches(Key::less, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "Ctrl+<"));
    assert!(!matches(Key::less, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "Ctrl+,"));
    assert!(matches(Key::B, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "Ctrl+Shift+B"));
    // A Latin key needs no display to look up.
    assert_eq!(latin(Key::s, 39), Key::s);
    assert!(valid("Ctrl+<"));
    assert!(!valid("Ctrl+Shift+,"));
    assert!(!valid("Ctrl+Shift+1"));
    assert!(!matches(Key::a, ModifierType::empty(), ""));
    assert!(!valid("K"));
    assert!(!valid("Ctrl+Foo"));
    assert!(!valid("Ctrl+K+L"));
    assert!(terminal_owns("Ctrl+K"));
    assert!(!terminal_owns("Ctrl+1"));
    assert!(!terminal_owns("Ctrl+Shift+B"));
}
