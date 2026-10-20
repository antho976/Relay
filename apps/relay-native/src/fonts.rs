//! The same font files as Relay-2, registered only with this application's map.
use gtk4::prelude::*;

/// The colour tokens every appearance mode defines, in the order of `PALETTES`' columns.
pub const TOKENS: [&str; 10] =
    ["wall", "console", "slab", "wash", "screen", "ink", "secondary", "edge", "faint", "strong"];
/// `@screen`'s column: the terminal and editor background.
pub const SCREEN: usize = 4;
/// Each appearance mode's tokens. The window's CSS (`load_appearance`) and the editor
/// schemes written below both come from this one table.
const PALETTES: [(&str, [&str; 10]); 3] = [
    // DESIGN.md's palette is Dark, the default: ground, ground, surface, selected, the
    // terminals' screen, ink, ink-2, line, ink-3, line-strong. Matte lifts its grounds a step;
    // OLED sets them on true black. The inks are the same in all three. `screen` paints the
    // terminals and the code editor and keeps each mode's earlier value: the terminals are not
    // part of the warm rework.
    ("matte", ["#171614", "#171614", "#1e1d1b", "#23211f", "#0a0a0b", "#ede9e2", "#b5b0a8", "#292725", "#8c877f", "#2c2a28"]),
    ("dark", ["#131211", "#131211", "#1a1917", "#1f1d1b", "#08090a", "#ede9e2", "#b5b0a8", "#242220", "#8c877f", "#262422"]),
    ("oled", ["#000000", "#000000", "#0f0e0d", "#161513", "#000000", "#ede9e2", "#b5b0a8", "#1c1b19", "#8c877f", "#211f1d"]),
];

/// The status tokens, the same in every appearance mode: theme.css defines them for the
/// window, and the editor schemes take them from here. Change both.
const STATUS: [(&str, &str); 3] = [("live", "#2ec469"), ("held", "#e5382e"), ("waiting", "#f0a828")];

/// The tokens of `mode`; an unknown mode is matte.
pub fn palette(mode: &str) -> [&'static str; 10] {
    PALETTES.iter().find(|(name, _)| *name == mode).unwrap_or(&PALETTES[0]).1
}

/// The Code editor's style scheme for one appearance mode: relay-editor.xml with each
/// `@token` replaced by that mode's colour.
fn editor_scheme(mode: &str, colors: [&str; 10]) -> String {
    let mut scheme = include_str!("../resources/relay-editor.xml").replace("RELAY_MODE", mode);
    // No token name is a prefix of another, so each `@token` replaces whole.
    for (token, color) in TOKENS.iter().zip(colors).chain(STATUS.iter().map(|(t, c)| (t, *c))) {
        scheme = scheme.replace(&format!("@{token}"), color);
    }
    scheme
}

pub fn install(window: &gtk4::ApplicationWindow) {
    let Some(map) = window.pango_context().font_map() else {
        return;
    };
    let directory = glib::user_cache_dir().join("relay-v4/fonts");
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        std::fs::create_dir_all(&directory)?;
        // The copies on disk are this executable's own when its stamp says so: they are then
        // only checked for size, not read back (556 KB) and compared before the first frame.
        let stamp_path = directory.join("stamp");
        let stamp = build_stamp();
        let stamped = stamp.is_some() && std::fs::read_to_string(&stamp_path).ok() == stamp;
        let styles = directory.join("styles");
        let current = |path: &std::path::Path, bytes: &[u8]| {
            if stamped {
                std::fs::metadata(path).is_ok_and(|m| m.len() == bytes.len() as u64)
            } else {
                std::fs::read(path).ok().as_deref() == Some(bytes)
            }
        };
        let mut wrote = !stamped;
        macro_rules! register {
            ($($name:literal),+ $(,)?) => {$(
                let bytes = include_bytes!(concat!("../resources/fonts/", $name));
                let path = directory.join($name);
                if !current(&path, bytes.as_slice()) {
                    std::fs::write(&path, bytes)?;
                    wrote = true;
                }
                map.add_font_file(&path)?;
            )+};
        }
        register!(
            "fira-sans-latin-400-normal.ttf",
            "fira-sans-latin-500-normal.ttf",
            "fira-sans-latin-600-normal.ttf",
            "fira-sans-condensed-latin-500-normal.ttf",
            "fira-sans-condensed-latin-600-normal.ttf",
            "fira-sans-condensed-latin-700-normal.ttf",
            "fira-mono-latin-400-normal.ttf",
            "fira-mono-latin-500-normal.ttf",
            // The interface's faces (theme.css): Geist for text, Geist Mono for numbers,
            // keycaps and code. Fira Mono above stays the terminals' face.
            "geist-latin-400-normal.ttf",
            "geist-latin-500-normal.ttf",
            "geist-latin-600-normal.ttf",
            // Bold: a design's 700 (the B key, Display and Title) draws bold, not as 600 (TM-9).
            "geist-latin-700-normal.ttf",
            "geist-mono-latin-400-normal.ttf",
            "geist-mono-latin-500-normal.ttf",
            // The wordmark's face (start.rs and the title bar).
            "sora-latin-600-normal.ttf",
        );
        map.changed();
        window.pango_context().changed();
        window.set_font_map(Some(&map));
        std::fs::create_dir_all(&styles)?;
        for (mode, colors) in PALETTES {
            let path = styles.join(format!("relay-{mode}.xml"));
            // Stamped, the schemes are this build's too: they need not even be written out.
            if stamped && path.is_file() {
                continue;
            }
            let scheme = editor_scheme(mode, colors);
            if std::fs::read_to_string(&path).ok().as_deref() != Some(&scheme) {
                std::fs::write(path, scheme)?;
                wrote = true;
            }
        }
        if let Some(stamp) = stamp.filter(|_| wrote) {
            std::fs::write(&stamp_path, stamp)?;
        }
        use sourceview5::prelude::*;
        sourceview5::StyleSchemeManager::default().append_search_path(&styles.to_string_lossy());
        Ok(())
    })();
    if let Err(error) = result {
        tracing::warn!(%error, "Could not register bundled Relay fonts");
    }
}

/// The running executable's size and modification time. The fonts and schemes are built
/// into it, so while it is unchanged the copies it wrote need no second look.
fn build_stamp() -> Option<String> {
    let meta = std::fs::metadata(std::env::current_exe().ok()?).ok()?;
    let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(format!("{} {}.{:09}\n", meta.len(), modified.as_secs(), modified.subsec_nanos()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn editor_schemes_resolve_every_token() {
        for (mode, colors) in super::PALETTES {
            let scheme = super::editor_scheme(mode, colors);
            assert!(!scheme.contains('@'), "{mode}: unresolved token in relay-editor.xml");
            assert!(scheme.contains(&format!("background=\"{}\"", colors[super::SCREEN])));
        }
    }
}
