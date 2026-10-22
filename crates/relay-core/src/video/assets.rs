//! A video's assets (`video.asset.add`, `video.asset.list`): the pictures, clips, sounds and fonts
//! under its `public/`, which the code names with `staticFile()`. A file comes in copied, or linked
//! where it is (a long clip, no second copy); a name another file has gets a number unless the call
//! says to replace it, and the answer says which. What Relay makes itself (a voice line, music, a
//! capture) keeps how it was made beside it, `<name>.json`.

use relay_bus::error::BusError;
use relay_bus::ops::video::{FontAdded, VideoAsset};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Most bytes one file may have (a long clip).
pub const MOST_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// Most files `video.asset.list` answers.
const MOST_LISTED: usize = 500;

/// What a file is, by its extension.
pub fn kind(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "avif" => "image",
        "mp4" | "webm" | "mov" | "mkv" | "m4v" => "video",
        "mp3" | "wav" | "ogg" | "m4a" | "aac" | "flac" | "opus" => "audio",
        "woff2" | "woff" | "ttf" | "otf" => "font",
        _ => "other",
    }
}

/// A file name safe in a URL and on any disk: letters, digits, dots, dashes and underscores.
pub fn safe_name(raw: &str) -> String {
    let name: String = raw.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '-' }).collect();
    let name = name.trim_matches(|c| c == '.' || c == '-').chars().take(120).collect::<String>();
    if name.is_empty() { "asset".into() } else { name }
}

/// The name a file takes: `name` (given or its own) made safe, with the source's extension when
/// the name has none.
pub fn target_name(source: &Path, name: Option<&str>) -> String {
    let original = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut name = safe_name(name.unwrap_or(&original));
    if !name.contains('.') {
        if let Some((_, ext)) = original.rsplit_once('.') {
            name = format!("{name}.{}", safe_name(ext));
        }
    }
    name
}

/// Whether `a` and `b` hold the same bytes (or `a` is a link to `b`).
fn same(a: &Path, b: &Path) -> bool {
    if fs::canonicalize(a).ok().is_some_and(|ra| fs::canonicalize(b).ok().is_some_and(|rb| ra == rb)) {
        return true;
    }
    let (Ok(ma), Ok(mb)) = (fs::metadata(a), fs::metadata(b)) else { return false };
    if ma.len() != mb.len() {
        return false;
    }
    // Large files compared in pieces, so two clips are never both held whole.
    use std::io::Read;
    let (Ok(mut fa), Ok(mut fb)) = (fs::File::open(a), fs::File::open(b)) else { return false };
    let (mut ba, mut bb) = (vec![0u8; 1 << 20], vec![0u8; 1 << 20]);
    loop {
        let (Ok(na), Ok(nb)) = (fa.read(&mut ba), fb.read(&mut bb)) else { return false };
        if na != nb || ba[..na] != bb[..nb] {
            return false;
        }
        if na == 0 {
            return true;
        }
    }
}

/// How to add one file.
pub struct Add<'a> {
    pub source: &'a Path,
    /// Under `public/`: `assets` or `assets/fonts`.
    pub folder: &'a str,
    pub name: Option<&'a str>,
    pub overwrite: bool,
    pub link: bool,
}

/// Add one file to the video in `dir`.
pub fn add(dir: &Path, add: &Add) -> Result<VideoAsset, BusError> {
    let source = add.source;
    let meta = fs::metadata(source).map_err(|e| BusError::invalid("video.asset", format!("{}: {e}", source.display())))?;
    if !meta.is_file() {
        return Err(BusError::invalid("video.asset", format!("{} is not a file", source.display())));
    }
    if meta.len() > MOST_BYTES {
        return Err(BusError::invalid("video.asset", format!("{} is over 4 GiB", source.display())));
    }
    let folder = dir.join("public").join(add.folder);
    fs::create_dir_all(&folder).map_err(|e| BusError::unavailable("video.folder", format!("{}: {e}", folder.display())))?;
    // The file already in its place (its own path, or a link to it): nothing to add. Writing a
    // link or a copy over it would put a link to itself, or nothing, where the file was.
    // (Relay's own files on their way in wait in hidden folders under public/: those are added.)
    if let (Ok(real), Ok(public)) = (fs::canonicalize(source), fs::canonicalize(dir.join("public"))) {
        let staged = real.strip_prefix(&public).is_ok_and(|rel| rel.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')));
        if real.starts_with(&public) && !staged && real.is_file() {
            return Ok(describe(dir, &real, None, false, true));
        }
    }
    let wanted = target_name(source, add.name);
    let (stem, ext) = wanted.rsplit_once('.').map_or((wanted.clone(), String::new()), |(s, e)| (s.to_string(), format!(".{e}")));
    let mut target = folder.join(&wanted);
    let mut renamed_from = None;
    let mut replaced = false;
    let taken = |p: &Path| p.symlink_metadata().is_ok();
    if taken(&target) && !same(&target, source) {
        if add.overwrite {
            replaced = true;
        } else {
            // Never written over unasked: a name taken by other content gets a number, and the
            // answer says so.
            let mut n = 2;
            while taken(&target) && !same(&target, source) {
                target = folder.join(format!("{stem}-{n}{ext}"));
                n += 1;
            }
            renamed_from = Some(format!("{}/{wanted}", add.folder));
        }
    }
    let unchanged = taken(&target) && same(&target, source) && !replaced
        && (add.link == target.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()));
    if !unchanged {
        let part = folder.join(format!(".{}.part", target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
        let _ = fs::remove_file(&part);
        let made = if add.link {
            let real = fs::canonicalize(source).map_err(|e| BusError::invalid("video.asset", format!("{}: {e}", source.display())))?;
            link(&real, &part)
        } else {
            fs::copy(source, &part).map(|_| ())
        };
        made.and_then(|_| fs::rename(&part, &target)).map_err(|e| {
            let _ = fs::remove_file(&part);
            BusError::unavailable("video.asset", format!("could not add {}: {e}", source.display()))
        })?;
    }
    Ok(describe(dir, &target, renamed_from, replaced, unchanged))
}

#[cfg(unix)]
fn link(real: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(real, at)
}

#[cfg(not(unix))]
fn link(real: &Path, at: &Path) -> std::io::Result<()> {
    fs::hard_link(real, at)
}

/// What a file under `dir/public/` is.
pub fn describe(dir: &Path, path: &Path, renamed_from: Option<String>, replaced: bool, unchanged: bool) -> VideoAsset {
    let public = dir.join("public");
    let rel = path.strip_prefix(&public).unwrap_or(path).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let kind = kind(&name);
    let bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let linked = path.symlink_metadata().ok().filter(|m| m.file_type().is_symlink())
        .and_then(|_| fs::read_link(path).ok()).map(|p| p.display().to_string());
    let (w, h) = if kind == "image" && bytes <= 64 * 1024 * 1024 {
        fs::read(path).ok().and_then(|b| crate::picture::sniff(&b)).map_or((None, None), |p| (Some(p.w), Some(p.h)))
    } else {
        (None, None)
    };
    let made = fs::read(sidecar(path)).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    VideoAsset { static_file: rel, path: path.display().to_string(), bytes, kind: kind.into(), w, h, renamed_from, replaced, unchanged, linked, made }
}

/// Where how a file was made is kept: `<name>.json` beside it.
pub fn sidecar(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!("{name}.json"))
}

/// Every file under `dir/public/`, by name; not dot-files, nor the `.json` kept beside one.
pub fn list(dir: &Path) -> Vec<VideoAsset> {
    let public = dir.join("public");
    let mut found = Vec::new();
    let mut stack = vec![public.clone()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = fs::read_dir(&at) else { continue };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = e.path();
            if path.is_dir() && !e.file_type().is_ok_and(|t| t.is_symlink()) {
                stack.push(path);
            } else if !(name.ends_with(".json") && Path::new(name.trim_end_matches(".json")).extension().is_some() && path.with_file_name(name.trim_end_matches(".json")).exists()) {
                found.push(path);
            }
        }
    }
    found.sort();
    found.truncate(MOST_LISTED);
    found.iter().map(|p| describe(dir, p, None, false, false)).collect()
}

/// A face fontconfig knows: its file, its CSS weight (or range, for a variable font) and whether
/// it is italic.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    pub file: PathBuf,
    pub weight: (u32, u32),
    pub italic: bool,
}

/// fontconfig's weight as CSS's (fontconfig.h: thin 0, extralight 40, light 50, book 75,
/// regular 80, medium 100, semibold 180, bold 200, extrabold 205, black 210).
fn css_weight(fc: f64) -> u32 {
    const SCALE: [(f64, u32); 10] = [(0.0, 100), (40.0, 200), (50.0, 300), (75.0, 350), (80.0, 400), (100.0, 500), (180.0, 600), (200.0, 700), (205.0, 800), (210.0, 900)];
    SCALE.iter().min_by(|a, b| (a.0 - fc).abs().total_cmp(&(b.0 - fc).abs())).map_or(400, |s| s.1)
}

/// The faces `fc-list` lists in its `file|weight|slant` lines.
pub fn parse_faces(text: &str) -> Vec<Face> {
    text.lines().filter_map(|line| {
        let mut parts = line.split('|');
        let (file, weight, slant) = (parts.next()?.trim(), parts.next()?.trim(), parts.next()?.trim());
        if file.is_empty() {
            return None;
        }
        let weight = if let Some(range) = weight.strip_prefix('[').and_then(|w| w.strip_suffix(']')) {
            let (a, b) = range.split_once(' ')?;
            (css_weight(a.parse().ok()?), css_weight(b.parse().ok()?))
        } else {
            let w = css_weight(weight.parse().ok()?);
            (w, w)
        };
        let italic = slant.trim_start_matches('[').split(' ').next().and_then(|s| s.parse::<f64>().ok()).is_some_and(|s| s >= 100.0);
        Some(Face { file: PathBuf::from(file), weight, italic })
    }).collect()
}

/// The installed faces of `family`, from fontconfig.
pub fn faces(family: &str) -> Result<Vec<Face>, BusError> {
    let fc = which::which("fc-list").map_err(|_| BusError::unavailable("video.font", "fc-list is not installed (fontconfig): add the font's files by path instead"))?;
    // A pattern's own marks (`-` size, `,` another family, `:` a property) escaped in the name.
    let escaped: String = family.chars().flat_map(|c| if matches!(c, '\\' | '-' | ',' | ':') { vec!['\\', c] } else { vec![c] }).collect();
    let out = crate::proc::output_with_timeout(Command::new(fc).arg("--format=%{file}|%{weight}|%{slant}\n").arg(format!(":family={escaped}")), Duration::from_secs(20))
        .map_err(|e| BusError::unavailable("video.font", format!("fc-list: {e}")))?
        .ok_or_else(|| BusError::unavailable("video.font", "fc-list did not answer within 20 seconds"))?;
    let mut found = parse_faces(&String::from_utf8_lossy(&out.stdout));
    found.retain(|f| matches!(kind(&f.file.to_string_lossy()), "font"));
    found.sort_by(|a, b| a.file.cmp(&b.file));
    found.dedup_by(|a, b| a.file == b.file);
    Ok(found)
}

/// For each weight wanted (in upright and, with `italic`, italic), the face that has it: an exact
/// one first, else a variable face whose range holds it. Each face once, with the weights it
/// stands for; and what none has.
pub fn pick(faces: &[Face], weights: &[u32], italic: bool) -> (Vec<(Face, Vec<u32>)>, Vec<String>) {
    let mut chosen: Vec<(Face, Vec<u32>)> = Vec::new();
    let mut missing = Vec::new();
    let styles: &[bool] = if italic { &[false, true] } else { &[false] };
    for &style in styles {
        for &w in weights {
            let exact = faces.iter().find(|f| f.italic == style && f.weight == (w, w));
            let ranged = faces.iter().find(|f| f.italic == style && f.weight.0 < f.weight.1 && (f.weight.0..=f.weight.1).contains(&w));
            match exact.or(ranged) {
                Some(face) => match chosen.iter_mut().find(|(f, _)| f.file == face.file) {
                    Some((_, ws)) => ws.push(w),
                    None => chosen.push((face.clone(), vec![w])),
                },
                None => missing.push(if style { format!("{w} italic") } else { w.to_string() }),
            }
        }
    }
    (chosen, missing)
}

/// The `loadFonts(…)` call for faces added as `static_files`.
pub fn load_call(family: &str, added: &[(String, &Face)]) -> String {
    let family_js = serde_json::to_string(family).unwrap_or_default();
    let lines: Vec<String> = added.iter().map(|(file, face)| {
        let weight = if face.weight.0 == face.weight.1 { face.weight.0.to_string() } else { format!("{} {}", face.weight.0, face.weight.1) };
        let style = if face.italic { ", style: 'italic'" } else { "" };
        format!("  {{ family: {family_js}, file: {}, weight: '{weight}'{style} }},", serde_json::to_string(file).unwrap_or_default())
    }).collect();
    format!("import {{ loadFonts }} from '@relay/video';\n\nloadFonts(\n{}\n);", lines.join("\n"))
}

/// The family's faces added: what `video.asset.add {font}` answers beside the files.
pub fn font_added(family: &str, added: &[(String, &Face)], missing: Vec<String>) -> FontAdded {
    let mut faces: Vec<String> = added.iter().map(|(_, f)| {
        let w = if f.weight.0 == f.weight.1 { f.weight.0.to_string() } else { format!("{}-{}", f.weight.0, f.weight.1) };
        if f.italic { format!("{w} italic") } else { w }
    }).collect();
    faces.dedup();
    FontAdded { family: family.to_string(), faces, missing, load: load_call(family, added) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_files_get_safe_names_and_kinds() {
        assert_eq!(safe_name("My Logo (final).PNG"), "My-Logo--final-.PNG");
        assert_eq!(safe_name("../../etc/passwd"), "etc-passwd");
        assert_eq!(kind("clip.MOV"), "video");
        assert_eq!(kind("Inter.woff2"), "font");
        assert_eq!(kind("song.mp3"), "audio");
        assert_eq!(kind("notes.txt"), "other");
        assert_eq!(target_name(Path::new("/x/sweep.wav"), Some("sfx-sweep")), "sfx-sweep.wav");
    }

    #[test]
    fn a_taken_name_is_numbered_and_said_unless_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        let (a, b) = (src.path().join("sweep.wav"), src.path().join("other").join("sweep.wav"));
        fs::create_dir_all(b.parent().unwrap()).unwrap();
        fs::write(&a, b"first").unwrap();
        fs::write(&b, b"second").unwrap();
        let add_one = |source: &Path, overwrite: bool, link: bool| add(dir.path(), &Add { source, folder: "assets", name: None, overwrite, link }).unwrap();
        let first = add_one(&a, false, false);
        assert_eq!((first.static_file.as_str(), first.renamed_from.as_deref()), ("assets/sweep.wav", None));
        let again = add_one(&a, false, false);
        assert!(again.unchanged, "the same content is the same asset: {again:?}");
        let clash = add_one(&b, false, false);
        assert_eq!(clash.static_file, "assets/sweep-2.wav");
        assert_eq!(clash.renamed_from.as_deref(), Some("assets/sweep.wav"), "the rename is said");
        let replaced = add_one(&b, true, false);
        assert_eq!((replaced.static_file.as_str(), replaced.replaced), ("assets/sweep.wav", true));
        assert_eq!(fs::read(dir.path().join("public/assets/sweep.wav")).unwrap(), b"second");
        // A link points at the file where it is, and the listing says so.
        let linked = add(dir.path(), &Add { source: &a, folder: "assets", name: Some("big"), overwrite: false, link: true }).unwrap();
        assert_eq!(linked.static_file, "assets/big.wav");
        assert_eq!(linked.linked.as_deref(), Some(fs::canonicalize(&a).unwrap().display().to_string().as_str()));
        let listed: Vec<String> = list(dir.path()).into_iter().map(|a| a.static_file).collect();
        assert_eq!(listed, ["assets/big.wav", "assets/sweep-2.wav", "assets/sweep.wav"]);
        // The asset itself, or a link to it, added again (linked or not) is left as it is: never
        // a link to itself where the file was.
        let own = dir.path().join("public/assets/sweep.wav");
        for link in [false, true] {
            let again = add(dir.path(), &Add { source: &own, folder: "assets", name: None, overwrite: true, link }).unwrap();
            assert!(again.unchanged && again.linked.is_none(), "{again:?}");
            assert_eq!(fs::read(&own).unwrap(), b"second");
        }
    }

    #[test]
    fn fontconfig_faces_are_read_and_picked_by_weight() {
        let text = "/f/NotoSerif-Regular.ttf|80|0\n/f/NotoSerif-Bold.ttf|200|0\n/f/NotoSerif-Italic.ttf|80|100\n/f/NotoSerif-Medium.ttf|100|0\n/v/Inter.ttf|[0 210]|0\n";
        let faces = parse_faces(text);
        assert_eq!(faces[0], Face { file: "/f/NotoSerif-Regular.ttf".into(), weight: (400, 400), italic: false });
        assert!(faces[2].italic);
        assert_eq!(faces[4].weight, (100, 900));
        let (chosen, missing) = pick(&faces[..4], &[400, 500, 700], true);
        let files: Vec<&str> = chosen.iter().map(|(f, _)| f.file.to_str().unwrap()).collect();
        assert_eq!(files, ["/f/NotoSerif-Regular.ttf", "/f/NotoSerif-Medium.ttf", "/f/NotoSerif-Bold.ttf", "/f/NotoSerif-Italic.ttf"]);
        assert_eq!(missing, ["500 italic", "700 italic"]);
        // A variable face stands for every weight in its range, once.
        let (chosen, missing) = pick(&faces[4..], &[300, 400, 700], false);
        assert_eq!((chosen.len(), chosen[0].1.len(), missing.len()), (1, 3, 0));
        let call = load_call("Noto Serif", &[("assets/fonts/NotoSerif-Bold.ttf".into(), &faces[1])]);
        assert!(call.contains("{ family: \"Noto Serif\", file: \"assets/fonts/NotoSerif-Bold.ttf\", weight: '700' }"), "{call}");
    }
}
