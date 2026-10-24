//! Relay's icon set (`design.icon.save`, `design.icon.list`, `design.icon.delete`): outline icons
//! saved once, by an agent or the person, for every project's designs. Built-in ones stay in
//! `relay_bus::design::ICONS`. An icon node that names a saved one copies its path into its own
//! `icon_path` when the edit runs ([`saved_path`]), so a design draws the same on any machine
//! and after the icon is changed or deleted.
//!
//! An SVG is taken apart here ([`svg_path`]): its shapes joined into one path and fitted from
//! its viewBox onto the 24-unit grid.

use crate::engine::{Ctx, Engine, IntoBus};
use crate::handlers::notes::actor_name;
use relay_bus::design::path_data::{self, Segment};
use relay_bus::design::{valid_id, ICONS};
use relay_bus::error::BusError;
use relay_bus::ops::design::{
    IconDelete, IconDeleteIn, IconDeleteOut, IconList, IconListOut, IconSave, IconSaveIn, IconSaveOut, SavedIcon, ICON_KEYWORDS_MAX,
    ICON_KEYWORD_LEN,
};
use relay_bus::ErrorKind;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::json;

const ICON_ROW: &str = "SELECT name, path, keywords, updated_by, updated_at FROM design_icons";

fn icon_row(r: &Row) -> rusqlite::Result<SavedIcon> {
    let keywords: String = r.get(2)?;
    Ok(SavedIcon {
        name: r.get(0)?,
        path: r.get(1)?,
        keywords: keywords.split_whitespace().map(str::to_string).collect(),
        updated_by: r.get(3)?,
        updated_at: r.get(4)?,
    })
}

fn saved(conn: &Connection, name: &str) -> Result<Option<SavedIcon>, BusError> {
    conn.prepare_cached(&format!("{ICON_ROW} WHERE name = ?1")).bus()?.query_row([name], icon_row).optional().bus()
}

/// The path of the icon saved as `name`, for an icon node that names it.
pub(super) fn saved_path(conn: &Connection, name: &str) -> Result<Option<String>, BusError> {
    conn.prepare_cached("SELECT path FROM design_icons WHERE name = ?1").bus()?.query_row([name], |r| r.get(0)).optional().bus()
}

fn invalid(field: &str, why: impl Into<String>) -> BusError {
    BusError::invalid("design.icon.save", why.into()).with_details(json!({"field": field}))
}

pub(super) fn register(e: &mut Engine) {
    e.register::<IconSave>(|ctx: &mut Ctx, p: IconSaveIn| {
        if !valid_id(&p.name) {
            return Err(invalid("name", format!("name: {:?} is not a name: 1 to 64 letters, digits, '-', '_', '.' or ':'", p.name)));
        }
        if ICONS.contains(&p.name.as_str()) {
            return Err(invalid("name", format!("name: {:?} is one of Relay's own icons; give yours another name", p.name)));
        }
        let mut warnings = Vec::new();
        let path = match (p.path.as_deref(), p.svg.as_deref()) {
            (Some(d), None) => path_data::normalize(d).map_err(|why| invalid("path", format!("path: {why}")))?,
            (None, Some(svg)) => {
                let (d, filled) = svg_path(svg).map_err(|why| invalid("svg", format!("svg: {why}")))?;
                if filled {
                    warnings.push("the SVG looks filled rather than outlined: an icon node draws only its outline; a filled picture belongs in an image node (design.asset.add)".to_string());
                }
                d
            }
            _ => return Err(invalid("path", "give path (SVG path data on the 24-unit grid) or svg (an SVG document), not both")),
        };
        // What is stored is read again by every icon node naming it: a number too large to write
        // back ("inf"), or repeats spelled out past the length cap, would make it unreadable.
        if p.path.is_some() {
            path_data::parse(&path).map_err(|why| invalid("path", format!("path: written out, it does not read back: {why}")))?;
        }
        let keywords = p.keywords.unwrap_or_default();
        if keywords.len() > ICON_KEYWORDS_MAX {
            return Err(invalid("keywords", format!("keywords: at most {ICON_KEYWORDS_MAX}")));
        }
        let mut words: Vec<String> = Vec::with_capacity(keywords.len());
        for word in &keywords {
            let word = word.trim().to_lowercase();
            if word.is_empty() || word.chars().count() > ICON_KEYWORD_LEN || word.contains(char::is_whitespace) {
                return Err(invalid("keywords", format!("keywords: {word:?}: each is one word of 1 to {ICON_KEYWORD_LEN} characters")));
            }
            if !words.contains(&word) {
                words.push(word);
            }
        }
        let before = saved(ctx.tx(), &p.name)?;
        if before.is_some() && p.replace != Some(true) {
            return Err(BusError::new(ErrorKind::Conflict, "design.icon_exists", format!("an icon named {:?} is saved already", p.name))
                .with_hint("pass replace: true to change it, or give yours another name; design.icon.list shows it"));
        }
        let by = actor_name(ctx)?;
        ctx.tx().prepare_cached(
            "INSERT INTO design_icons(name, path, keywords, created_by, updated_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?5)
             ON CONFLICT(name) DO UPDATE SET path = ?2, keywords = ?3, updated_by = ?4, updated_at = ?5",
        ).bus()?.execute(params![p.name, path, words.join(" "), by, ctx.now]).bus()?;
        match &before {
            Some(old) => ctx.set_undo("design.icon.save", json!({"name": old.name, "path": old.path, "keywords": old.keywords, "replace": true}), None),
            None => ctx.set_undo("design.icon.delete", json!({"name": p.name}), None),
        }
        let icon = saved(ctx.tx(), &p.name)?.ok_or_else(|| BusError::internal("the icon saved is not there"))?;
        Ok(IconSaveOut { icon, replaced: before.is_some(), warnings })
    });
    // Unlocked (D149): one read of a table of short rows.
    e.register_unlocked::<IconList>(|ctx, p| {
        let wanted = p.query.as_deref().map(str::to_lowercase);
        let hit = |text: &str| wanted.as_deref().is_none_or(|w| text.to_lowercase().contains(w));
        let saved = ctx.read(|conn| {
            conn.prepare_cached(&format!("{ICON_ROW} ORDER BY name")).bus()?
                .query_map([], icon_row).bus()?
                .collect::<rusqlite::Result<Vec<_>>>().bus()
        })?;
        Ok(IconListOut {
            built_in: ICONS.iter().filter(|name| hit(name)).map(|name| name.to_string()).collect(),
            saved: saved.into_iter().filter(|icon| hit(&icon.name) || icon.keywords.iter().any(|k| hit(k))).collect(),
        })
    });
    e.register::<IconDelete>(|ctx: &mut Ctx, p: IconDeleteIn| {
        let Some(before) = saved(ctx.tx(), &p.name)? else { return Ok(IconDeleteOut { deleted: false }) };
        ctx.tx().prepare_cached("DELETE FROM design_icons WHERE name = ?1").bus()?.execute([&p.name]).bus()?;
        ctx.set_undo("design.icon.save", json!({"name": before.name, "path": before.path, "keywords": before.keywords}), None);
        Ok(IconDeleteOut { deleted: true })
    });
}

/// An SVG document's shapes as one path on the 24-unit grid, and whether it looks filled (no
/// stroke anywhere, so its shapes are drawn in their fill). Refused when it has no shapes, a shape that
/// does not read, a `transform`, a `<use>` or text, or a viewBox (else width and height) that is
/// not positive.
pub(super) fn svg_path(svg: &str) -> Result<(String, bool), String> {
    let (mut view, mut segments, mut stroked) = (None, Vec::new(), false);
    for tag in tags(svg) {
        let attr = |key: &str| attribute(tag.attrs, key);
        let num = |key: &str| -> Result<f64, String> {
            let text = attr(key).unwrap_or("0");
            text.trim().trim_end_matches("px").parse::<f64>().ok().filter(|v| v.is_finite())
                .ok_or_else(|| format!("<{}> {key}={text:?} is not a number", tag.name))
        };
        if attr("transform").is_some() {
            return Err(format!("<{}> has a transform; flatten it first (most editors export without one)", tag.name));
        }
        if attr("stroke").is_some_and(|s| s != "none") || attr("stroke-width").is_some() {
            stroked = true;
        }
        let d = match tag.name {
            "svg" => {
                view = match attr("viewBox") {
                    Some(v) => {
                        let parts: Vec<f64> = v.split([' ', ',']).filter(|p| !p.is_empty()).filter_map(|p| p.parse().ok()).collect();
                        match parts[..] {
                            [x, y, w, h] => Some((x, y, w, h)),
                            _ => return Err(format!("viewBox {v:?} is not four numbers")),
                        }
                    }
                    None => Some((0.0, 0.0, num("width")?, num("height")?)),
                };
                continue;
            }
            "use" | "text" | "image" => return Err(format!("<{}> cannot be an outline: draw it as a path", tag.name)),
            "path" => attr("d").unwrap_or("").to_string(),
            "line" => format!("M{} {}L{} {}", num("x1")?, num("y1")?, num("x2")?, num("y2")?),
            "polyline" | "polygon" => {
                let points: Vec<&str> = attr("points").unwrap_or("").split([' ', ',', '\n', '\t']).filter(|p| !p.is_empty()).collect();
                if points.len() < 4 || points.len() % 2 == 1 {
                    return Err(format!("<{}> points are not pairs of numbers", tag.name));
                }
                let pairs: Vec<String> = points.chunks(2).map(|p| format!("{} {}", p[0], p[1])).collect();
                let close = if tag.name == "polygon" { "Z" } else { "" };
                format!("M{}{close}", pairs.join("L"))
            }
            "circle" | "ellipse" => {
                let (cx, cy) = (num("cx")?, num("cy")?);
                let (rx, ry) = if tag.name == "circle" { (num("r")?, num("r")?) } else { (num("rx")?, num("ry")?) };
                if rx <= 0.0 || ry <= 0.0 {
                    continue;
                }
                format!("M{} {cy}a{rx} {ry} 0 1 0 {} 0a{rx} {ry} 0 1 0 {} 0", cx - rx, 2.0 * rx, -2.0 * rx)
            }
            "rect" => {
                let (x, y, w, h) = (num("x")?, num("y")?, num("width")?, num("height")?);
                let r = if attr("rx").is_some() { num("rx")? } else { num("ry")? }.clamp(0.0, w.min(h) / 2.0);
                if r > 0.0 {
                    let (iw, ih) = (w - 2.0 * r, h - 2.0 * r);
                    format!("M{} {y}h{iw}a{r} {r} 0 0 1 {r} {r}v{ih}a{r} {r} 0 0 1 -{r} {r}h{}a{r} {r} 0 0 1 -{r} -{r}v{}a{r} {r} 0 0 1 {r} -{r}Z", x + r, -iw, -ih)
                } else {
                    format!("M{x} {y}h{w}v{h}h{}Z", -w)
                }
            }
            _ => continue,
        };
        let mut shape = path_data::parse(&d).map_err(|why| format!("<{}>: {why}", tag.name))?;
        segments.append(&mut shape);
    }
    if segments.is_empty() {
        return Err("it holds no path, line, polyline, polygon, circle, ellipse or rect".into());
    }
    let (vx, vy, vw, vh) = view.unwrap_or((0.0, 0.0, 24.0, 24.0));
    if !(vw > 0.0 && vh > 0.0) {
        return Err(format!("its viewBox is {vw} by {vh}; it needs a size"));
    }
    let scale = 24.0 / vw.max(vh);
    let (dx, dy) = (-vx * scale + (24.0 - vw * scale) / 2.0, -vy * scale + (24.0 - vh * scale) / 2.0);
    let moved: &mut [Segment] = &mut segments;
    path_data::transform(moved, scale, dx, dy);
    let d = path_data::write(&segments);
    path_data::parse(&d).map_err(|why| format!("it makes path data too long: {why}"))?;
    // With no stroke anywhere, its shapes are drawn filled, which an outline icon does not do.
    Ok((d, !stroked))
}

/// One element's start tag: its name and what follows it up to `>`.
struct Tag<'a> {
    name: &'a str,
    attrs: &'a str,
}

/// The start tags of an XML document, comments and declarations aside.
fn tags(xml: &str) -> impl Iterator<Item = Tag<'_>> {
    let mut rest = xml;
    std::iter::from_fn(move || loop {
        let open = rest.find('<')?;
        rest = &rest[open + 1..];
        if rest.starts_with("!--") {
            rest = rest.find("-->").map_or("", |end| &rest[end + 3..]);
            continue;
        }
        let close = rest.find('>')?;
        let inside = &rest[..close];
        rest = &rest[close + 1..];
        if inside.starts_with(['/', '?', '!']) {
            continue;
        }
        let end = inside.find(|c: char| c.is_whitespace() || c == '/').unwrap_or(inside.len());
        let name = inside[..end].rsplit(':').next().unwrap_or("");
        return Some(Tag { name, attrs: &inside[end..] });
    })
}

/// Attribute `key` of a start tag's attributes, quoted either way; a `style` declaration of the
/// same name counts when the attribute is absent (`style="fill:none"`).
fn attribute<'a>(attrs: &'a str, key: &str) -> Option<&'a str> {
    let mut at = 0;
    while let Some(found) = attrs[at..].find(key) {
        let start = at + found;
        at = start + key.len();
        let before = attrs[..start].chars().next_back();
        if before.is_some_and(|c| !c.is_whitespace()) {
            continue;
        }
        let after = attrs[at..].trim_start();
        let Some(after) = after.strip_prefix('=') else { continue };
        let after = after.trim_start();
        let quote = after.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let value = &after[1..];
        return value.find(quote).map(|end| &value[..end]);
    }
    let style = attribute_plain(attrs, "style")?;
    style.split(';').find_map(|decl| {
        let (name, value) = decl.split_once(':')?;
        (name.trim() == key).then_some(value.trim())
    })
}

/// [`attribute`] without the `style` fallback.
fn attribute_plain<'a>(attrs: &'a str, key: &str) -> Option<&'a str> {
    let at = attrs.find(&format!(" {key}=")).or_else(|| attrs.find(&format!("\t{key}=")))?;
    let after = attrs[at + key.len() + 2..].trim_start();
    let quote = after.chars().next()?;
    let value = &after[1..];
    value.find(quote).map(|end| &value[..end])
}

#[cfg(test)]
mod tests {
    use super::svg_path;

    #[test]
    fn an_svg_becomes_one_path_on_the_grid() {
        let lucide = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <!-- a comment <path d="M0 0"/> -->
            <path d="M3 6h18"/><line x1="3" y1="12" x2="21" y2="12"/><polyline points="3,18 12,18 21 18"/>
            <circle cx="12" cy="12" r="2"/><rect x="2" y="2" width="20" height="20" rx="2"/></svg>"#;
        let (d, filled) = svg_path(lucide).unwrap();
        assert!(!filled);
        assert_eq!(d, "M3 6 h18 M3 12 L21 12 M3 18 L12 18 L21 18 M10 12 a2 2 0 1 0 4 0 a2 2 0 1 0 -4 0 \
            M4 2 h16 a2 2 0 0 1 2 2 v16 a2 2 0 0 1 -2 2 h-16 a2 2 0 0 1 -2 -2 v-16 a2 2 0 0 1 2 -2 Z");
        // Another grid is fitted onto 24, centred along its shorter side.
        let (d, _) = svg_path(r#"<svg viewBox="0 0 256 128"><polygon points="0 0 256 0 256 128"/></svg>"#).unwrap();
        assert_eq!(d, "M0 6 L24 6 L24 18 Z");
        let (d, _) = svg_path(r#"<svg width="48px" height="48px"><path d="M0 0L48 48"/></svg>"#).unwrap();
        assert_eq!(d, "M0 0 L24 24");
        // Filled, with no stroke anywhere.
        let (_, filled) = svg_path(r##"<svg viewBox="0 0 24 24"><path fill="#000" d="M0 0h24v24z"/></svg>"##).unwrap();
        assert!(filled);
        let (_, filled) = svg_path(r#"<svg viewBox="0 0 24 24" style="fill:none;stroke:#000"><path d="M0 0h24"/></svg>"#).unwrap();
        assert!(!filled, "a style counts");
    }

    #[test]
    fn what_cannot_be_an_outline_is_refused() {
        for (svg, why) in [
            (r#"<svg viewBox="0 0 24 24"></svg>"#, "holds no path"),
            (r#"<svg viewBox="0 0 24 24"><g transform="scale(2)"><path d="M0 0h1"/></g></svg>"#, "transform"),
            (r##"<svg viewBox="0 0 24 24"><use href="#a"/></svg>"##, "<use> cannot"),
            (r#"<svg viewBox="0 0 24 24"><path d="L0 0"/></svg>"#, "starts with a move"),
            (r#"<svg viewBox="0 0 x 24"><path d="M0 0h1"/></svg>"#, "four numbers"),
            (r#"<svg viewBox="0 0 0 24"><path d="M0 0h1"/></svg>"#, "needs a size"),
            (r#"<svg viewBox="0 0 24 24"><circle cx="a" r="2"/></svg>"#, "not a number"),
        ] {
            let error = svg_path(svg).unwrap_err();
            assert!(error.contains(why), "{svg}: {error}");
        }
    }
}
