use super::*;
use base64::Engine as _;

const MAX_BYTES: u64 = 16 * 1024 * 1024;
const MAX_EDGE: i32 = 4096;

pub(super) fn is_image(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tif" | "tiff" | "ico" | "svg"
            )
        })
}

pub(super) struct Preview {
    pub root: gtk::Box,
    pub picture: gtk::Picture,
    title: gtk::Label,
    message: gtk::Label,
}
impl Preview {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let title = label("", "mono");
        title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        title.set_hexpand(true);
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        heading.add_css_class("toolbar");
        heading.append(&title);
        root.append(&heading);
        let picture = gtk::Picture::new();
        picture.set_widget_name("project-image");
        picture.set_can_shrink(true);
        // At its own size, fitted only when larger than the pane: a 160x120 icon drawn at 890px
        // was all blur.
        picture.set_content_fit(gtk::ContentFit::ScaleDown);
        picture.set_hexpand(true);
        picture.set_vexpand(true);
        picture.set_margin_top(16);
        picture.set_margin_bottom(16);
        picture.set_margin_start(16);
        picture.set_margin_end(16);
        root.append(&picture);
        let message = label("", "dim");
        message.set_widget_name("project-image-message");
        message.set_wrap(true);
        message.set_selectable(true);
        message.set_justify(gtk::Justification::Center);
        message.set_xalign(0.5);
        message.set_halign(gtk::Align::Center);
        message.set_valign(gtk::Align::Center);
        message.set_vexpand(true);
        message.set_margin_start(24);
        message.set_margin_end(24);
        root.append(&message);
        Self {
            root,
            picture,
            title,
            message,
        }
    }

    fn loading(&self, path: &str) {
        self.title.set_text(path);
        self.title.set_tooltip_text(Some(path));
        self.picture.set_alternative_text(Some(path));
        self.clear();
        self.message.set_text("Loading image…");
        self.message.set_visible(true);
        self.picture.set_visible(false);
    }

    pub fn clear(&self) {
        self.picture.set_paintable(None::<&gtk::gdk::Paintable>);
    }
}

struct Decoded {
    pixels: Vec<u8>,
    width: i32,
    height: i32,
    stride: usize,
    alpha: bool,
    original: (i32, i32),
}

// Decode on a worker and transfer only pixel bytes to GTK. Limit the displayed
// resolution so a large asset cannot allocate an equally large GPU texture.
fn decode(value: Value) -> Result<Decoded, String> {
    decode_to(value, MAX_EDGE)
}

fn decode_to(value: Value, max_edge: i32) -> Result<Decoded, String> {
    if value["truncated"] == true {
        return Err("Image exceeds the 16 MiB preview limit.".into());
    }
    let bytes = if let Some(encoded) = value["bytes_b64"].as_str() {
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| e.to_string())?
    } else if let Some(text) = value["text"].as_str() {
        text.as_bytes().to_vec()
    } else {
        return Err("The file did not contain image data.".into());
    };
    // gdk-pixbuf 2.44 (glycin) reports the size only once the whole image is decoded, so the
    // limit is checked against the header first, then again against what the decoder saw.
    let declared = header_size(&bytes)?;
    if declared.is_some_and(|(width, height)| width.saturating_mul(height) > MAX_PIXELS) {
        return Err(TOO_LARGE.into());
    }
    let loader = gtk::gdk_pixbuf::PixbufLoader::new();
    let dimensions = Rc::new(Cell::new((0, 0)));
    let prepared = dimensions.clone();
    loader.connect_size_prepared(move |loader, width, height| {
        prepared.set((width, height));
        let scale = (max_edge as f64 / width.max(height).max(1) as f64).min(1.);
        loader.set_size(
            (width as f64 * scale).max(1.) as i32,
            (height as f64 * scale).max(1.) as i32,
        );
    });
    let read: Result<(), String> = bytes.chunks(8192).try_for_each(|chunk| {
        loader.write(chunk).map_err(|e| e.to_string())?;
        let (width, height) = dimensions.get();
        if i64::from(width) * i64::from(height) > MAX_PIXELS as i64 {
            return Err(TOO_LARGE.into());
        }
        Ok(())
    });
    let closed = loader.close().map_err(|e| e.to_string());
    read?;
    closed?;
    // An SVG is drawn at the preview size whatever canvas it declares.
    let (width, height) = dimensions.get();
    if declared.is_some() && i64::from(width) * i64::from(height) > MAX_PIXELS as i64 {
        return Err(TOO_LARGE.into());
    }
    let pixbuf = loader
        .pixbuf()
        .ok_or("The image format could not be decoded.")?;
    let pixbuf = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);
    Ok(Decoded {
        pixels: pixbuf.read_pixel_bytes().as_ref().to_vec(),
        width: pixbuf.width(),
        height: pixbuf.height(),
        stride: pixbuf.rowstride() as usize,
        alpha: pixbuf.has_alpha(),
        original: dimensions.get(),
    })
}

const MAX_PIXELS: u64 = 100_000_000;
const TOO_LARGE: &str = "Image dimensions exceed the 100 megapixel preview limit.";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

/// An image's declared width and height, read from its header before any decoder runs.
/// `None` is an SVG. Content that is none of the previewed formats is refused, so it cannot
/// reach whatever other decoder is installed.
pub(crate) fn header_size(bytes: &[u8]) -> Result<Option<(u64, u64)>, String> {
    let size = if bytes.starts_with(PNG) {
        png_size(bytes)
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        jpeg_size(bytes)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        int(bytes, 6, 2, false).zip(int(bytes, 8, 2, false))
    } else if bytes.starts_with(b"BM") {
        bmp_size(bytes)
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        webp_size(bytes)
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        tiff_size(bytes)
    } else if bytes.starts_with(&[0, 0, 1, 0]) || bytes.starts_with(&[0, 0, 2, 0]) {
        ico_size(bytes)
    } else if bytes
        .strip_prefix(b"\xef\xbb\xbf")
        .unwrap_or(bytes)
        .trim_ascii_start()
        .starts_with(b"<")
    {
        return Ok(None);
    } else {
        return Err("This is not a PNG, JPEG, GIF, BMP, WebP, TIFF, ICO or SVG image.".into());
    };
    size.map(Some).ok_or_else(|| "The image header could not be read.".into())
}

/// An unsigned integer of `len` bytes at `at`.
fn int(bytes: &[u8], at: usize, len: usize, big_endian: bool) -> Option<u64> {
    let field = bytes.get(at..at.checked_add(len)?)?;
    let fold = |n: u64, byte: &u8| n << 8 | u64::from(*byte);
    Some(if big_endian {
        field.iter().fold(0, fold)
    } else {
        field.iter().rev().fold(0, fold)
    })
}

fn png_size(bytes: &[u8]) -> Option<(u64, u64)> {
    if bytes.get(12..16)? != b"IHDR" {
        return None;
    }
    int(bytes, 16, 4, true).zip(int(bytes, 20, 4, true))
}

/// The first frame header (SOFn) after any number of other segments.
fn jpeg_size(bytes: &[u8]) -> Option<(u64, u64)> {
    let mut at = 2;
    loop {
        if *bytes.get(at)? != 0xff {
            return None;
        }
        while *bytes.get(at)? == 0xff {
            at += 1;
        }
        let marker = *bytes.get(at)?;
        at += 1;
        match marker {
            0x01 | 0xd0..=0xd7 => {}
            0xd9 | 0xda => return None,
            0xc0..=0xcf if !matches!(marker, 0xc4 | 0xc8 | 0xcc) => {
                return int(bytes, at + 5, 2, true).zip(int(bytes, at + 3, 2, true));
            }
            _ => at += int(bytes, at, 2, true)? as usize,
        }
    }
}

/// A DIB header's width and height at `at`; a negative height means top-down rows.
fn dib_size(bytes: &[u8], at: usize) -> Option<(u64, u64)> {
    if int(bytes, at, 4, false)? == 12 {
        return int(bytes, at + 4, 2, false).zip(int(bytes, at + 6, 2, false));
    }
    let signed = |at| int(bytes, at, 4, false).map(|n| u64::from((n as u32 as i32).unsigned_abs()));
    signed(at + 4).zip(signed(at + 8))
}

fn bmp_size(bytes: &[u8]) -> Option<(u64, u64)> {
    dib_size(bytes, 14)
}

fn webp_size(bytes: &[u8]) -> Option<(u64, u64)> {
    match bytes.get(12..16)? {
        b"VP8 " => Some((int(bytes, 26, 2, false)? & 0x3fff, int(bytes, 28, 2, false)? & 0x3fff)),
        b"VP8L" => {
            let bits = int(bytes, 21, 4, false)?;
            Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
        }
        b"VP8X" => Some((int(bytes, 24, 3, false)? + 1, int(bytes, 27, 3, false)? + 1)),
        _ => None,
    }
}

/// ImageWidth and ImageLength from the first IFD, the page gdk-pixbuf shows.
fn tiff_size(bytes: &[u8]) -> Option<(u64, u64)> {
    let big = bytes[0] == b'M';
    let ifd = int(bytes, 4, 4, big)? as usize;
    let (mut width, mut height) = (None, None);
    for entry in 0..int(bytes, ifd, 2, big)? as usize {
        let at = ifd + 2 + entry * 12;
        let value = match int(bytes, at + 2, 2, big)? {
            3 => int(bytes, at + 8, 2, big)?,
            4 => int(bytes, at + 8, 4, big)?,
            _ => continue,
        };
        match int(bytes, at, 2, big)? {
            256 => width = Some(value),
            257 => height = Some(value),
            _ => {}
        }
    }
    width.zip(height)
}

/// The largest entry; each holds a PNG or a DIB, whose header can claim more than the
/// directory's byte-sized edges.
fn ico_size(bytes: &[u8]) -> Option<(u64, u64)> {
    let mut largest: Option<(u64, u64)> = None;
    for entry in 0..int(bytes, 4, 2, false)? as usize {
        let at = 6 + entry * 16;
        let edge = |n: u64| if n == 0 { 256 } else { n };
        let listed = (edge(int(bytes, at, 1, false)?), edge(int(bytes, at + 1, 1, false)?));
        let offset = int(bytes, at + 12, 4, false)? as usize;
        let image = bytes.get(offset..)?;
        let (width, height) = if image.starts_with(PNG) {
            png_size(image)?
        } else {
            // A DIB in an icon stacks the AND mask under the image, doubling its height.
            dib_size(image, 0).map(|(width, height)| (width, height / 2))?
        };
        let size = (width.max(listed.0), height.max(listed.1));
        if largest.is_none_or(|(w, h)| size.0.saturating_mul(size.1) > w.saturating_mul(h)) {
            largest = Some(size);
        }
    }
    largest
}

impl Decoded {
    fn texture(self) -> gtk::gdk::MemoryTexture {
        let format = if self.alpha {
            gtk::gdk::MemoryFormat::R8g8b8a8
        } else {
            gtk::gdk::MemoryFormat::R8g8b8
        };
        gtk::gdk::MemoryTexture::new(
            self.width,
            self.height,
            format,
            &glib::Bytes::from_owned(self.pixels),
            self.stride,
        )
    }
}

/// The longest edge of a hover thumbnail.
const THUMB_EDGE: i32 = 256;
/// Hover thumbnails kept per window; a tree full of assets is browsed, not memorised.
const THUMB_CACHE: usize = 64;
const HOVER_DELAY: std::time::Duration = std::time::Duration::from_millis(350);

#[derive(Clone)]
struct Thumb {
    texture: Option<gtk::gdk::MemoryTexture>,
    caption: String,
}

type ThumbCache = (Vec<String>, std::collections::HashMap<String, Thumb>);
thread_local! {
    static THUMBS: RefCell<ThumbCache> = RefCell::new((Vec::new(), std::collections::HashMap::new()));
}

fn cached(key: &str) -> Option<Thumb> {
    THUMBS.with(|cache| cache.borrow().1.get(key).cloned())
}

fn remember(key: String, thumb: Thumb) {
    THUMBS.with(|cache| {
        let (order, map) = &mut *cache.borrow_mut();
        if map.insert(key.clone(), thumb).is_none() {
            order.push(key);
        }
        while order.len() > THUMB_CACHE {
            let oldest = order.remove(0);
            map.remove(&oldest);
        }
    });
}

fn caption(decoded: &Decoded, size: u64, path: &str) -> String {
    let format = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_uppercase();
    format!("{} × {} · {} · {format}", decoded.original.0, decoded.original.1, file_size(size))
}

/// A file's size, as every other page writes one.
fn file_size(size: u64) -> String {
    relay_client::figures::bytes(size)
}

/// Fill a hover card from a thumbnail, or its error.
fn show_thumb(card: &gtk::Box, picture: &gtk::Picture, note: &gtk::Label, thumb: &Thumb) {
    card.remove_css_class("loading");
    match &thumb.texture {
        Some(texture) => {
            picture.set_paintable(Some(texture));
            // A tiny icon is drawn larger so its shape reads, never past 4×.
            let (width, height) = (texture.width(), texture.height());
            let scale = (96. / width.max(height).max(1) as f64).clamp(1., 4.);
            picture.set_size_request((width as f64 * scale) as i32, (height as f64 * scale) as i32);
            picture.set_visible(true);
            note.remove_css_class("error");
        }
        None => {
            picture.set_visible(false);
            note.add_css_class("error");
        }
    }
    note.set_text(&thumb.caption);
}

impl Editor {
    /// Show a delayed thumbnail beside any row naming an image, as VS Code's explorer does.
    /// `stamp` (size and modification time, when the caller has them) keeps the cache honest.
    pub(super) fn bind_image_hover(
        self: &Rc<Self>,
        ui: &Rc<Ui>,
        row: &impl IsA<gtk::Widget>,
        path: &str,
        stamp: String,
        size: Option<u64>,
    ) {
        if !is_image(path) {
            return;
        }
        let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
        let shown: Rc<RefCell<Option<gtk::Popover>>> = Rc::default();
        let hide = {
            let pending = pending.clone();
            let shown = shown.clone();
            move || {
                if let Some(source) = pending.borrow_mut().take() {
                    source.remove();
                }
                let popover = shown.borrow_mut().take();
                if let Some(popover) = popover {
                    popover.popdown();
                }
            }
        };
        let motion = gtk::EventControllerMotion::new();
        let target = row.as_ref().downgrade();
        let editor = Rc::downgrade(self);
        let weak_ui = Rc::downgrade(ui);
        let path = path.to_owned();
        motion.connect_enter(move |_, _, _| {
            if pending.borrow().is_some() || shown.borrow().is_some() {
                return;
            }
            let fired = pending.clone();
            let shown = shown.clone();
            let target = target.clone();
            let editor = editor.clone();
            let weak_ui = weak_ui.clone();
            let path = path.clone();
            let stamp = stamp.clone();
            let source = glib::timeout_add_local_once(HOVER_DELAY, move || {
                // The source has fired; it must not be removed again.
                fired.borrow_mut().take();
                let (Some(row), Some(editor), Some(ui)) =
                    (target.upgrade(), editor.upgrade(), weak_ui.upgrade())
                else {
                    return;
                };
                if !row.is_mapped() {
                    return;
                }
                let popover = gtk::Popover::new();
                popover.add_css_class("image-hover");
                popover.set_autohide(false);
                popover.set_has_arrow(false);
                popover.set_can_focus(false);
                popover.set_position(gtk::PositionType::Right);
                popover.set_offset(6, 0);
                let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
                card.add_css_class("image-hover-card");
                card.add_css_class("loading");
                let name = label(
                    std::path::Path::new(&path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(&path),
                    "image-hover-name",
                );
                name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                name.set_max_width_chars(36);
                card.append(&name);
                let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
                frame.add_css_class("image-hover-frame");
                frame.set_halign(gtk::Align::Center);
                let picture = gtk::Picture::new();
                picture.set_can_shrink(false);
                picture.set_content_fit(gtk::ContentFit::Contain);
                picture.set_visible(false);
                picture.set_alternative_text(Some(&path));
                frame.append(&picture);
                card.append(&frame);
                let note = label("Loading preview…", "image-hover-meta");
                note.set_wrap(true);
                note.set_max_width_chars(36);
                card.append(&note);
                popover.set_child(Some(&card));
                popover.set_parent(&row);
                // The card says what the row's tooltip says, and more: one at a time.
                row.set_has_tooltip(false);
                let closing = shown.clone();
                popover.connect_closed(move |popover| {
                    closing.borrow_mut().take();
                    if let Some(row) = popover.parent() {
                        row.set_has_tooltip(row.tooltip_text().is_some() || row.tooltip_markup().is_some());
                    }
                    popover.unparent();
                });
                *shown.borrow_mut() = Some(popover.clone());
                popover.popup();
                let project = ui.project.get();
                let worktree = editor.worktree.borrow().clone();
                let key = format!("{project}\0{worktree}\0{path}\0{stamp}");
                if let Some(thumb) = cached(&key) {
                    show_thumb(&card, &picture, &note, &thumb);
                    return;
                }
                // The tree already knows the size: a file the engine would only truncate is not
                // worth 16 MiB on the wire to learn that.
                if size.is_some_and(|size| size > MAX_BYTES) {
                    let thumb = Thumb { texture: None, caption: "No preview · Image exceeds the 16 MiB preview limit.".into() };
                    remember(key, thumb.clone());
                    show_thumb(&card, &picture, &note, &thumb);
                    return;
                }
                let path = path.clone();
                glib::spawn_future_local(async move {
                    let result = crate::client::Client::image_read(&ui.rt, ui.path.clone(),
                        json!({"project_id":project,"worktree":optional_scope(&worktree),"path":path,"max_bytes":MAX_BYTES}))
                        .await;
                    // Only what the file itself decides is kept: a timeout or a lost connection
                    // is retried on the next hover rather than shown until the file changes.
                    let decoded = match result {
                        Ok(value) => {
                            // The pointer moved on while the bytes came: no one would see the
                            // thumbnail, and a full-size decode is the expensive half.
                            if !popover.is_visible() {
                                return;
                            }
                            let size = value["size"].as_u64().unwrap_or(0);
                            match ui.rt.spawn_blocking(move || decode_to(value, THUMB_EDGE)).await {
                                Ok(decoded) => Ok((decoded, size)),
                                Err(error) => Err(error.to_string()),
                            }
                        }
                        Err(error) => Err(readable(&error)),
                    };
                    let (thumb, keep) = match decoded {
                        Ok((Ok(decoded), size)) => (Thumb {
                            caption: caption(&decoded, size, &path),
                            texture: Some(decoded.texture()),
                        }, true),
                        Ok((Err(error), _)) => (Thumb { texture: None, caption: format!("No preview · {error}") }, true),
                        Err(error) => (Thumb { texture: None, caption: format!("No preview · {error}") }, false),
                    };
                    if keep {
                        remember(key, thumb.clone());
                    }
                    show_thumb(&card, &picture, &note, &thumb);
                });
            });
            *pending.borrow_mut() = Some(source);
        });
        let leave = hide.clone();
        motion.connect_leave(move |_| leave());
        row.add_controller(motion);
        // A tree rebuild, or the click that opens the file, must not leave a card floating.
        let unmapped = hide.clone();
        row.connect_unmap(move |_| unmapped());
        let click = gtk::GestureClick::new();
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        click.connect_pressed(move |_, _, _, _| hide());
        row.add_controller(click);
    }

    pub(super) fn open_image(self: &Rc<Self>, ui: &Rc<Ui>, path: String) {
        self.revision.set(self.revision.get() + 1);
        let revision = self.revision.get();
        let project = ui.project.get();
        let worktree = self.worktree.borrow().clone();
        self.image_mode.set(true);
        self.diff.set(false);
        self.before_scroll.set_visible(false);
        self.buffer.set_text("");
        self.buffer.set_modified(false);
        self.original.borrow_mut().clear();
        *self.path.borrow_mut() = path.clone();
        self.project.set(project);
        self.image.loading(&path);
        self.note_opened(ui, &path);
        self.facts.set_text("");
        self.position.set_text("Image preview");
        self.find.hide();
        self.set_busy(true);
        let e = self.clone();
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let result = crate::client::Client::image_read(&ui.rt, ui.path.clone(),
                json!({"project_id":project,"worktree":optional_scope(&worktree),"path":path,"max_bytes":MAX_BYTES}))
                .await.map_err(|err| readable(&err));
            if e.revision.get() != revision || !e.matches(&ui, project, &worktree) {
                return;
            }
            let size = result
                .as_ref()
                .ok()
                .and_then(|v| v["size"].as_u64())
                .unwrap_or(0);
            let decoded = match result {
                Ok(value) => ui
                    .rt
                    .spawn_blocking(move || decode(value))
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|v| v),
                Err(error) => Err(error),
            };
            if e.revision.get() != revision || !e.matches(&ui, project, &worktree) {
                return;
            }
            e.set_busy(false);
            match decoded {
                Ok(decoded) => {
                    let original = decoded.original;
                    let texture = decoded.texture();
                    e.image.picture.set_paintable(Some(&texture));
                    e.image.picture.set_visible(true);
                    e.image.message.set_visible(false);
                    e.position.set_text(&format!(
                        "{} × {} px · {} · Image preview",
                        original.0,
                        original.1,
                        file_size(size)
                    ));
                }
                Err(error) => {
                    e.image
                        .message
                        .set_text(&format!("Cannot preview image\n{error}"));
                    e.position.set_text("Image preview unavailable");
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_formats_decode_without_a_display_and_large_images_are_scaled() {
        let pixbuf =
            gtk::gdk_pixbuf::Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 5000, 8)
                .unwrap();
        pixbuf.fill(0x3377aaff);
        for format in ["png", "jpeg", "bmp", "tiff"] {
            let bytes = pixbuf.save_to_bufferv(format, &[]).unwrap();
            assert_eq!(header_size(&bytes), Ok(Some((5000, 8))), "{format}");
            let image = decode(
                json!({"bytes_b64":base64::engine::general_purpose::STANDARD.encode(bytes)}),
            )
            .unwrap();
            assert_eq!(image.original, (5000, 8));
            assert_eq!(image.width, MAX_EDGE);
            assert!(image.height > 0 && image.height <= 8);
        }
    }

    #[test]
    fn hover_thumbnails_are_bounded_and_captioned_with_the_original_size() {
        let pixbuf =
            gtk::gdk_pixbuf::Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, true, 8, 1200, 600)
                .unwrap();
        pixbuf.fill(0x3377aaff);
        let bytes = pixbuf.save_to_bufferv("png", &[]).unwrap();
        let thumb = decode_to(
            json!({"bytes_b64":base64::engine::general_purpose::STANDARD.encode(&bytes)}),
            THUMB_EDGE,
        )
        .unwrap();
        assert_eq!((thumb.width, thumb.height), (THUMB_EDGE, THUMB_EDGE / 2));
        assert_eq!(caption(&thumb, 3 * 1048576 / 2, "art/hero.png"), "1200 × 600 · 1.5 MiB · PNG");
        assert_eq!(caption(&thumb, 10, "a.webp"), "1200 × 600 · 1 KiB · WEBP");
        for n in 0..THUMB_CACHE + 5 {
            remember(n.to_string(), Thumb { texture: None, caption: String::new() });
        }
        assert!(cached("0").is_none() && cached(&(THUMB_CACHE + 4).to_string()).is_some());
        THUMBS.with(|cache| assert_eq!(cache.borrow().1.len(), THUMB_CACHE));
    }

    #[test]
    fn svg_and_invalid_or_oversized_images_have_explicit_results() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20"><rect width="30" height="20" fill="#3377aa"/></svg>"##;
        assert_eq!(decode(json!({"text":svg})).unwrap().original, (30, 20));
        assert!(decode(json!({"text":"not an image"})).is_err());
        assert!(decode(json!({"truncated":true}))
            .err()
            .unwrap()
            .contains("16 MiB"));
        assert!(is_image("ART/Preview.PNG") && is_image("photo.JPEG") && is_image("asset.webp"));
        // The header alone refuses a huge image: only IHDR, no pixel data, is needed.
        let mut huge = PNG.to_vec();
        huge.extend_from_slice(&[0, 0, 0, 13]);
        huge.extend_from_slice(b"IHDR");
        huge.extend_from_slice(&11000u32.to_be_bytes());
        huge.extend_from_slice(&10000u32.to_be_bytes());
        huge.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        let encoded = base64::engine::general_purpose::STANDARD.encode(&huge);
        assert_eq!(decode(json!({"bytes_b64":encoded})).err().as_deref(), Some(TOO_LARGE));
        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&[0x10, 0x27, 0x10, 0x27]);
        assert_eq!(header_size(&gif), Ok(Some((10000, 10000))));
        let mut webp = b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0".to_vec();
        webp.extend_from_slice(&[0x0f, 0x27, 0, 0x0f, 0x27, 0]);
        assert_eq!(header_size(&webp), Ok(Some((10000, 10000))));
        assert_eq!(header_size(b"\xef\xbb\xbf\n<svg/>"), Ok(None));
        assert!(header_size(b"\0\0\0\x0cjP  \r\n\x87\n").is_err());
        assert!(!is_image("image.png.rs"));
    }
}
