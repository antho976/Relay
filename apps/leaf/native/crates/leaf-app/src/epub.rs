//! A book as the reader reads it: sections (an EPUB's spine, or a text file cut into parts), each a
//! list of blocks (headings, paragraphs, quotes, list items, pictures), each block one string with
//! styled spans and links. Built once per opening on a worker thread ([`open`]), never on the UI
//! thread: an EPUB is a zip of XHTML read with `zip` and `quick-xml`.
//!
//! Offsets inside a block are byte offsets into its text (always on a character boundary), so a
//! place or a highlight is (section, block, byte) and parsing the same file again gives the same
//! numbers.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};

/// Span styles, as bits.
pub const ITALIC: u8 = 1;
pub const BOLD: u8 = 2;
pub const LINK: u8 = 4;
/// A small raised mark (a footnote's number).
pub const SUP: u8 = 8;

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// h1 to h6.
    Heading(u8),
    Para,
    Quote,
    /// A list item: how deep, and its mark ("•", "3.").
    Item(u8, String),
    /// A picture: its path inside the book's zip, and what it shows in words.
    Image(String, String),
    /// A break between parts (hr).
    Rule,
    /// Preformatted text: lines kept as they are.
    Pre,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub style: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// Somewhere in this book: (section, block).
    Inside(u32, u32),
    /// A web address (not followed: Leaf opens nothing outside the book).
    Outside(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub start: u32,
    pub end: u32,
    pub target: Target,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub kind: Kind,
    pub text: String,
    /// Styled runs (only the styled ones; the rest is plain), in order, not overlapping.
    pub spans: Vec<Span>,
    pub links: Vec<Link>,
    /// Characters of the book before this block (for progress and time left).
    pub at: u64,
}

impl Block {
    /// The style at byte `i`.
    pub fn style_at(&self, i: u32) -> u8 {
        self.spans.iter().find(|s| s.start <= i && i < s.end).map_or(0, |s| s.style)
    }

    pub fn chars(&self) -> u64 {
        self.text.chars().count() as u64
    }
}

#[derive(Clone, Debug, Default)]
pub struct Section {
    pub blocks: Vec<Block>,
    /// Its file inside the zip (for links into it).
    pub href: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TocEntry {
    pub title: String,
    pub level: u8,
    pub section: u32,
    pub block: u32,
}

#[derive(Default, Debug)]
pub struct Doc {
    pub title: String,
    pub author: String,
    pub sections: Vec<Section>,
    pub toc: Vec<TocEntry>,
    /// Characters in the whole book.
    pub chars: u64,
}

/// What the library needs of a file, read at import: no layout, no text.
#[derive(Debug, Default)]
pub struct Meta {
    pub title: String,
    pub author: String,
    /// The cover picture's bytes, when the book names one.
    pub cover: Option<Vec<u8>>,
}

impl Doc {
    pub fn block(&self, s: u32, b: u32) -> Option<&Block> {
        self.sections.get(s as usize)?.blocks.get(b as usize)
    }

    /// Characters before byte `byte` of block (s, b), from the book's start.
    pub fn char_at(&self, s: u32, b: u32, byte: u32) -> u64 {
        match self.block(s, b) {
            Some(bl) => bl.at + bl.text.get(..(byte as usize).min(bl.text.len())).map_or(0, |t| t.chars().count() as u64),
            None => self.chars,
        }
    }

    /// The contents entry a place is under (the last one at or before it).
    pub fn toc_index(&self, s: u32, b: u32) -> Option<usize> {
        self.toc.iter().rposition(|t| (t.section, t.block) <= (s, b))
    }

    /// The chapter a place is in, as (title, first char, end char): the contents entry over it,
    /// or the section when the book has no contents.
    pub fn chapter_span(&self, s: u32, b: u32) -> (String, u64, u64) {
        if let Some(i) = self.toc_index(s, b) {
            let t = &self.toc[i];
            let start = self.char_at(t.section, t.block, 0);
            let end = self.toc.get(i + 1).map_or(self.chars, |n| self.char_at(n.section, n.block, 0));
            return (t.title.clone(), start, end.max(start));
        }
        let start = self.char_at(s, 0, 0);
        let end = self.sections.get(s as usize + 1).map_or(self.chars, |_| self.char_at(s + 1, 0, 0));
        (String::new(), start, end.max(start))
    }

    /// Give every block its place in the book's characters.
    fn count(&mut self) {
        let mut at = 0u64;
        for s in &mut self.sections {
            for b in &mut s.blocks {
                b.at = at;
                at += b.chars();
            }
        }
        self.chars = at;
    }
}

/// Open a book's file: EPUB or plain text, by its extension.
pub fn open(path: &Path) -> Result<Doc, String> {
    match ext(path).as_str() {
        "epub" => open_epub(path),
        "txt" => {
            let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read the file: {e}"))?;
            Ok(parse_txt(&String::from_utf8_lossy(&bytes), &stem(path)))
        }
        other => Err(format!("Leaf can't read .{other} files")),
    }
}

pub fn ext(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// The file's name without its extension, tidied into a title ("thinking_fast-and-slow" →
/// "thinking fast and slow").
pub fn stem(path: &Path) -> String {
    let s = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled");
    let s: String = s.chars().map(|c| if c == '_' || c == '-' { ' ' } else { c }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.is_empty() { "Untitled".into() } else { s }
}

// Plain text

/// Paragraphs are separated by blank lines; a lone short line in capitals, or one that starts
/// "Chapter", is a heading. Cut into sections of a few hundred paragraphs (at headings when it can).
pub fn parse_txt(text: &str, title: &str) -> Doc {
    let mut doc = Doc { title: title.to_string(), ..Doc::default() };
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut sec = Section { href: "0".into(), ..Section::default() };
    for para in text.split("\n\n") {
        let lines: Vec<&str> = para.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            continue;
        }
        let joined = lines.join(" ");
        let heading = lines.len() == 1 && joined.chars().count() < 70 && (is_caps(&joined) || joined.to_ascii_lowercase().starts_with("chapter "));
        if heading && sec.blocks.len() > 40 || sec.blocks.len() >= 400 {
            doc.sections.push(std::mem::take(&mut sec));
            sec.href = doc.sections.len().to_string();
        }
        if heading {
            doc.toc.push(TocEntry { title: tidy_caps(&joined), level: 1, section: doc.sections.len() as u32, block: sec.blocks.len() as u32 });
        }
        let kind = if heading { Kind::Heading(2) } else { Kind::Para };
        sec.blocks.push(Block { kind, text: joined, spans: Vec::new(), links: Vec::new(), at: 0 });
    }
    if !sec.blocks.is_empty() || doc.sections.is_empty() {
        doc.sections.push(sec);
    }
    doc.count();
    doc
}

/// A title in capitals ("THE SIXTH BOOK") set in title case ("The Sixth Book"); others as they are.
pub fn tidy_caps(s: &str) -> String {
    if !is_caps(s) {
        return s.to_string();
    }
    const SMALL: [&str; 9] = ["a", "an", "and", "of", "the", "in", "on", "to", "or"];
    s.split(' ')
        .enumerate()
        .map(|(i, w)| {
            let lower = w.to_lowercase();
            // Roman numerals stay in capitals.
            if w.len() > 1 && w.chars().all(|c| "IVXLCDM.,:".contains(c)) {
                return w.to_string();
            }
            if i > 0 && SMALL.contains(&lower.as_str()) {
                return lower;
            }
            let mut ch = lower.chars();
            match ch.next() {
                Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A plain-text file's own "Title:" and "Author:" lines near its top (Project Gutenberg's way).
pub fn txt_meta(text: &str) -> (Option<String>, Option<String>) {
    let (mut title, mut author) = (None, None);
    for line in text.lines().take(80) {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("Title:") {
            title.get_or_insert_with(|| v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("Author:") {
            author.get_or_insert_with(|| v.trim().to_string());
        }
    }
    (title.filter(|t| !t.is_empty()), author.filter(|a| !a.is_empty()))
}

fn is_caps(s: &str) -> bool {
    let letters: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 3 && letters.iter().all(|c| c.is_uppercase())
}

// EPUB

struct Zip {
    z: zip::ZipArchive<std::fs::File>,
}

impl Zip {
    fn open(path: &Path) -> Result<Zip, String> {
        let f = std::fs::File::open(path).map_err(|e| format!("Couldn't read the file: {e}"))?;
        let z = zip::ZipArchive::new(f).map_err(|_| "This isn't an EPUB Leaf can open (not a zip file)".to_string())?;
        Ok(Zip { z })
    }

    /// An entry's bytes, at most `cap` of them.
    fn read(&mut self, name: &str, cap: u64) -> Option<Vec<u8>> {
        let name = name.trim_start_matches('/');
        // Some books percent-encode names in the OPF ("my%20file.xhtml").
        let i = self.z.index_for_name(name).or_else(|| self.z.index_for_name(&percent_decode(name)))?;
        let mut f = self.z.by_index(i).ok()?;
        if f.size() > cap {
            return None;
        }
        let mut out = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut out).ok()?;
        Some(out)
    }

    fn text(&mut self, name: &str) -> Option<String> {
        self.read(name, 16 << 20).map(|b| String::from_utf8_lossy(&b).into_owned())
    }
}

/// One picture from inside an EPUB (a book's figure), at most 16 MB.
pub fn read_entry(book: &Path, name: &str) -> Option<Vec<u8>> {
    Zip::open(book).ok()?.read(name, 16 << 20)
}

struct Opf {
    dir: String,
    title: String,
    author: String,
    /// id → (href from the zip's root, media type, properties).
    manifest: HashMap<String, (String, String, String)>,
    spine: Vec<String>,
    cover_id: Option<String>,
    ncx: Option<String>,
}

fn read_opf(z: &mut Zip) -> Result<Opf, String> {
    let container = z.text("META-INF/container.xml").ok_or("This EPUB has no container.xml")?;
    let mut opf_path = None;
    each_tag(&container, |name, a| {
        if name == "rootfile" && opf_path.is_none() {
            opf_path = attr(a, "full-path");
        }
    });
    let opf_path = opf_path.ok_or("This EPUB names no package file")?;
    let opf = z.text(&opf_path).ok_or("This EPUB's package file is missing")?;
    let dir = parent(&opf_path);
    let mut o = Opf { dir: dir.clone(), title: String::new(), author: String::new(), manifest: HashMap::new(), spine: Vec::new(), cover_id: None, ncx: None };
    // Titles and creators are text; the rest attributes.
    let mut reader = quick_xml::Reader::from_str(&opf);
    let mut open: Option<String> = None;
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let n = local(&e);
                if n == "title" || n == "creator" {
                    open = Some(n);
                    text.clear();
                } else {
                    opf_tag(&mut o, &n, &e);
                }
            }
            Ok(Event::Empty(e)) => opf_tag(&mut o, &local(&e), &e),
            Ok(Event::Text(t)) if open.is_some() => text.push_str(&t.decode().unwrap_or_default()),
            Ok(Event::GeneralRef(r)) if open.is_some() => text.push_str(&entity(&r.decode().unwrap_or_default())),
            Ok(Event::End(e)) => {
                let n = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if open.as_deref() == Some(n.as_str()) {
                    let v = collapse(&text);
                    if n == "title" && o.title.is_empty() {
                        o.title = v;
                    } else if n == "creator" && o.author.is_empty() {
                        o.author = v;
                    }
                    open = None;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    Ok(o)
}

fn opf_tag(o: &mut Opf, name: &str, e: &BytesStart) {
    let a = attrs(e);
    match name {
        "item" => {
            let (Some(id), Some(href)) = (attr(&a, "id"), attr(&a, "href")) else { return };
            let media = attr(&a, "media-type").unwrap_or_default();
            let props = attr(&a, "properties").unwrap_or_default();
            if props.split_whitespace().any(|p| p == "cover-image") {
                o.cover_id = Some(id.clone());
            }
            o.manifest.insert(id, (join(&o.dir, &href), media, props));
        }
        "itemref" => {
            if let Some(id) = attr(&a, "idref") {
                o.spine.push(id);
            }
        }
        "spine" => o.ncx = attr(&a, "toc"),
        "meta" => {
            if attr(&a, "name").as_deref() == Some("cover")
                && o.cover_id.is_none()
                && let Some(c) = attr(&a, "content")
            {
                o.cover_id = Some(c);
            }
        }
        _ => {}
    }
}

/// Title, author and cover of an EPUB, or of a text file (its name).
pub fn meta(path: &Path) -> Result<Meta, String> {
    if ext(path) != "epub" {
        // The first few KB of a text file may name it.
        let mut head = vec![0u8; 8192];
        let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut head)).unwrap_or(0);
        let (title, author) = if ext(path) == "pdf" { (None, None) } else { txt_meta(&String::from_utf8_lossy(&head[..n])) };
        return Ok(Meta { title: title.unwrap_or_else(|| stem(path)), author: author.unwrap_or_default(), cover: None });
    }
    let mut z = Zip::open(path)?;
    let o = read_opf(&mut z)?;
    let mut cover = o.cover_id.as_ref().and_then(|id| o.manifest.get(id)).and_then(|(href, ..)| z.read(href, 16 << 20));
    if cover.is_none() {
        // No cover named: an image whose name says cover.
        let guess = o.manifest.values().find(|(h, m, _)| m.starts_with("image/") && h.to_ascii_lowercase().contains("cover")).map(|(h, ..)| h.clone());
        cover = guess.and_then(|h| z.read(&h, 16 << 20));
    }
    let title = if o.title.is_empty() { stem(path) } else { o.title };
    Ok(Meta { title, author: o.author, cover })
}

fn open_epub(path: &Path) -> Result<Doc, String> {
    let mut z = Zip::open(path)?;
    let o = read_opf(&mut z)?;
    let mut doc = Doc { title: o.title.clone(), author: o.author.clone(), ..Doc::default() };
    let cover_href = o.cover_id.as_ref().and_then(|id| o.manifest.get(id)).map(|(h, ..)| h.clone());
    // Parse each spine item; ids are kept per file to resolve links after.
    let mut ids: Vec<HashMap<String, u32>> = Vec::new();
    let mut raw_links: Vec<Vec<Vec<String>>> = Vec::new();
    for id in &o.spine {
        let Some((href, media, _)) = o.manifest.get(id) else { continue };
        if !media.contains("html") && !media.contains("xml") {
            continue;
        }
        let Some(xhtml) = z.text(href) else { continue };
        let parsed = parse_xhtml(&xhtml, &parent(href));
        // The cover's own page (just the cover picture) is left out: the library shows it.
        if parsed.blocks.len() == 1
            && let Kind::Image(src, _) = &parsed.blocks[0].kind
            && Some(src) == cover_href.as_ref()
        {
            continue;
        }
        if parsed.blocks.is_empty() {
            continue;
        }
        doc.sections.push(Section { blocks: parsed.blocks, href: href.clone() });
        ids.push(parsed.ids);
        raw_links.push(parsed.hrefs);
    }
    if doc.sections.is_empty() {
        return Err("This EPUB has no text Leaf can read".into());
    }
    let by_href: HashMap<String, usize> = doc.sections.iter().enumerate().map(|(i, s)| (s.href.clone(), i)).collect();
    let lens: Vec<usize> = doc.sections.iter().map(|s| s.blocks.len()).collect();
    let resolve = |href: &str| -> Option<(u32, u32)> {
        let (file, frag) = href.split_once('#').unwrap_or((href, ""));
        let file = percent_decode(file);
        let s = *by_href.get(&file)?;
        let b = if frag.is_empty() { 0 } else { ids[s].get(&percent_decode(frag)).copied().unwrap_or(0) };
        let b = b.min(lens[s].saturating_sub(1) as u32);
        Some((s as u32, b))
    };
    // Links: their targets, now that every file's ids are known.
    let mut targets: Vec<Vec<Vec<Target>>> = Vec::new();
    for (s, sec) in doc.sections.iter().enumerate() {
        let mut per_block = Vec::new();
        for (b, block) in sec.blocks.iter().enumerate() {
            let mut t = Vec::new();
            for (i, _) in block.links.iter().enumerate() {
                let href = &raw_links[s][b][i];
                t.push(if href.contains("://") || href.starts_with("mailto:") {
                    Target::Outside(href.clone())
                } else {
                    let full = if href.starts_with('#') { format!("{}{href}", sec.href) } else { join(&parent(&sec.href), href) };
                    match resolve(&full) {
                        Some((ts, tb)) => Target::Inside(ts, tb),
                        None => Target::Outside(href.clone()),
                    }
                });
            }
            per_block.push(t);
        }
        targets.push(per_block);
    }
    for (s, sec) in doc.sections.iter_mut().enumerate() {
        for (b, block) in sec.blocks.iter_mut().enumerate() {
            for (i, l) in block.links.iter_mut().enumerate() {
                l.target = targets[s][b][i].clone();
            }
        }
    }
    // Contents: the EPUB 3 nav, else the NCX, else the headings.
    let nav_href = o.manifest.values().find(|(_, _, p)| p.split_whitespace().any(|x| x == "nav")).map(|(h, ..)| h.clone());
    let mut toc: Vec<(String, u8, String)> = Vec::new();
    if let Some(h) = &nav_href
        && let Some(x) = z.text(h)
    {
        toc = parse_nav(&x, &parent(h));
    }
    if toc.is_empty()
        && let Some(id) = &o.ncx
        && let Some((h, ..)) = o.manifest.get(id)
        && let Some(x) = z.text(h)
    {
        toc = parse_ncx(&x, &parent(h));
    }
    let mut seen = std::collections::HashSet::new();
    for (title, level, href) in toc {
        let title = tidy_caps(&title);
        if let Some((s, b)) = resolve(&href)
            && !title.is_empty()
            && seen.insert((s, b))
        {
            doc.toc.push(TocEntry { title, level, section: s, block: b });
        }
    }
    doc.toc.sort_by_key(|t| (t.section, t.block));
    if doc.toc.is_empty() {
        for (s, sec) in doc.sections.iter().enumerate() {
            for (b, block) in sec.blocks.iter().enumerate() {
                if let Kind::Heading(l) = block.kind
                    && l <= 3
                {
                    doc.toc.push(TocEntry { title: block.text.clone(), level: l.saturating_sub(1).max(1), section: s as u32, block: b as u32 });
                }
            }
        }
    }
    if doc.title.is_empty() {
        doc.title = stem(path);
    }
    doc.count();
    Ok(doc)
}

struct Parsed {
    blocks: Vec<Block>,
    ids: HashMap<String, u32>,
    /// Each block's links' hrefs, as written, in the order of its links.
    hrefs: Vec<Vec<String>>,
}

/// Elements that make a block of their own.
fn is_block(n: &str) -> bool {
    matches!(
        n,
        "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote" | "li" | "ul" | "ol" | "pre" | "figure" | "figcaption" | "section" | "article" | "aside" | "header" | "footer" | "nav" | "dt" | "dd" | "dl" | "tr" | "table" | "tbody" | "thead" | "body" | "center" | "address" | "main"
    )
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    hrefs: Vec<Vec<String>>,
    ids: HashMap<String, u32>,
    text: String,
    spans: Vec<Span>,
    links: Vec<Link>,
    link_hrefs: Vec<String>,
    /// Styles open now (a count per bit, so nested <i><i> work).
    italic: u32,
    bold: u32,
    sup: u32,
    /// Links open: (href, start byte).
    open_links: Vec<(String, u32)>,
    /// Block elements open, innermost last.
    stack: Vec<String>,
    /// Lists open: (ordered, next number).
    lists: Vec<(bool, u32)>,
    /// Inside <pre>: whitespace is kept.
    pre: u32,
    /// Inside head, script, style: nothing is read.
    skip: u32,
    /// The last thing added was whitespace (or nothing): another space is dropped.
    space: bool,
    /// The list item mark for the block being built.
    mark: Option<(u8, String)>,
}

impl Builder {
    fn style(&self) -> u8 {
        (if self.italic > 0 { ITALIC } else { 0 }) | (if self.bold > 0 { BOLD } else { 0 }) | (if self.sup > 0 { SUP } else { 0 }) | (if self.open_links.is_empty() { 0 } else { LINK })
    }

    fn push_text(&mut self, s: &str) {
        let style = self.style();
        let start = self.text.len() as u32;
        if self.pre > 0 {
            self.text.push_str(s);
        } else {
            for c in s.chars() {
                if c.is_whitespace() && c != '\u{a0}' {
                    if !self.space {
                        self.text.push(' ');
                        self.space = true;
                    }
                } else {
                    self.text.push(c);
                    self.space = false;
                }
            }
        }
        let end = self.text.len() as u32;
        if style != 0 && end > start {
            match self.spans.last_mut() {
                Some(l) if l.end == start && l.style == style => l.end = end,
                _ => self.spans.push(Span { start, end, style }),
            }
        }
    }

    fn kind(&self) -> Kind {
        for n in self.stack.iter().rev() {
            match n.as_str() {
                "h1" => return Kind::Heading(1),
                "h2" => return Kind::Heading(2),
                "h3" => return Kind::Heading(3),
                "h4" => return Kind::Heading(4),
                "h5" => return Kind::Heading(5),
                "h6" => return Kind::Heading(6),
                "pre" => return Kind::Pre,
                _ => {}
            }
        }
        if let Some((d, m)) = &self.mark {
            return Kind::Item(*d, m.clone());
        }
        if self.stack.iter().any(|n| n == "blockquote") {
            return Kind::Quote;
        }
        Kind::Para
    }

    /// End the block being built (if it holds any words).
    fn flush(&mut self) {
        // Links still open carry on into the next block.
        let open: Vec<(String, u32)> = std::mem::take(&mut self.open_links);
        for (href, start) in &open {
            if !href.is_empty() && *start != u32::MAX {
                self.close_link_at(href.clone(), *start);
            }
        }
        let lead = self.text.len() - self.text.trim_start().len();
        let text = self.text.trim().to_string();
        if !text.is_empty() {
            let len = text.len() as u32;
            let fix = |a: u32| a.saturating_sub(lead as u32).min(len);
            let spans = self.spans.iter().map(|s| Span { start: fix(s.start), end: fix(s.end), style: s.style }).filter(|s| s.end > s.start).collect();
            let mut links = Vec::new();
            let mut hrefs = Vec::new();
            for (l, h) in self.links.iter().zip(&self.link_hrefs) {
                let (a, b) = (fix(l.start), fix(l.end));
                if b > a {
                    links.push(Link { start: a, end: b, target: Target::Outside(String::new()) });
                    hrefs.push(h.clone());
                }
            }
            let kind = self.kind();
            self.blocks.push(Block { kind, text, spans, links, at: 0 });
            self.hrefs.push(hrefs);
            self.mark = None;
        }
        self.text.clear();
        self.spans.clear();
        self.links.clear();
        self.link_hrefs.clear();
        self.space = true;
        self.open_links = open.into_iter().map(|(h, s)| (h, if s == u32::MAX { s } else { 0 })).collect();
    }

    fn close_link_at(&mut self, href: String, start: u32) {
        let end = self.text.len() as u32;
        if end > start {
            self.links.push(Link { start, end, target: Target::Outside(String::new()) });
            self.link_hrefs.push(href);
        }
    }

    fn picture(&mut self, src: String, alt: String) {
        self.flush();
        self.blocks.push(Block { kind: Kind::Image(src, alt), text: String::new(), spans: Vec::new(), links: Vec::new(), at: 0 });
        self.hrefs.push(Vec::new());
    }
}

fn parse_xhtml(x: &str, dir: &str) -> Parsed {
    let mut b = Builder { space: true, ..Builder::default() };
    let mut reader = quick_xml::Reader::from_str(x);
    {
        let c = reader.config_mut();
        c.check_end_names = false;
        c.allow_unmatched_ends = true;
    }
    while let Ok(ev) = reader.read_event() {
        match ev {
            Event::Start(e) => {
                let n = local(&e);
                let a = attrs(&e);
                open_tag(&mut b, &n, &a, dir, false);
            }
            Event::Empty(e) => {
                let n = local(&e);
                let a = attrs(&e);
                open_tag(&mut b, &n, &a, dir, true);
            }
            Event::End(e) => {
                let n = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                close_tag(&mut b, &n);
            }
            Event::Text(t) if b.skip == 0 => {
                let s = t.decode().unwrap_or_default();
                b.push_text(&s);
            }
            Event::CData(t) if b.skip == 0 => {
                let s = String::from_utf8_lossy(&t).into_owned();
                b.push_text(&s);
            }
            Event::GeneralRef(r) if b.skip == 0 => {
                let s = match r.resolve_char_ref() {
                    Ok(Some(c)) => c.to_string(),
                    _ => entity(&r.decode().unwrap_or_default()),
                };
                // An entity is a character, never collapsed whitespace (&nbsp; stays).
                let keep = b.space;
                b.push_text(&s);
                if s == " " {
                    b.space = keep;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    b.flush();
    Parsed { blocks: b.blocks, ids: b.ids, hrefs: b.hrefs }
}

fn open_tag(b: &mut Builder, n: &str, a: &[(String, String)], dir: &str, empty: bool) {
    if matches!(n, "head" | "script" | "style" | "title") {
        if !empty {
            b.skip += 1;
        }
        return;
    }
    if b.skip > 0 {
        return;
    }
    if is_block(n) && !empty {
        b.flush();
    }
    if let Some(id) = attr(a, "id") {
        // Before the block is flushed: an id at a block's start belongs to the block to come,
        // one inside belongs to the block it is in; both are blocks.len() until it is pushed.
        b.ids.entry(id).or_insert(b.blocks.len() as u32);
    }
    match n {
        "br" => {
            if b.pre > 0 {
                b.text.push('\n');
            } else {
                b.text.push('\n');
                b.space = true;
            }
        }
        "hr" => {
            b.flush();
            b.blocks.push(Block { kind: Kind::Rule, text: String::new(), spans: Vec::new(), links: Vec::new(), at: 0 });
            b.hrefs.push(Vec::new());
        }
        "img" | "image" => {
            let src = attr(a, "src").or_else(|| attr(a, "href")).or_else(|| attr(a, "xlink:href"));
            if let Some(src) = src
                && !src.starts_with("data:")
            {
                b.picture(join(dir, &src), attr(a, "alt").unwrap_or_default());
            }
        }
        "i" | "em" | "cite" | "var" | "dfn" => b.italic += !empty as u32,
        "b" | "strong" => b.bold += !empty as u32,
        "sup" => b.sup += !empty as u32,
        "a" if !empty => {
            if let Some(h) = attr(a, "href") {
                let start = b.text.len() as u32;
                b.open_links.push((h, start));
            } else {
                b.open_links.push((String::new(), u32::MAX));
            }
        }
        _ if is_block(n) && !empty => {
            b.flush();
            match n {
                "ul" | "ol" => b.lists.push((n == "ol", 1)),
                "li" => {
                    let depth = b.lists.len().max(1) as u8;
                    let mark = match b.lists.last_mut() {
                        Some((true, k)) => {
                            let m = format!("{k}.");
                            *k += 1;
                            m
                        }
                        _ => "•".into(),
                    };
                    b.mark = Some((depth, mark));
                }
                "pre" => b.pre += 1,
                _ => {}
            }
            b.stack.push(n.to_string());
        }
        _ => {}
    }
}

fn close_tag(b: &mut Builder, n: &str) {
    if matches!(n, "head" | "script" | "style" | "title") {
        b.skip = b.skip.saturating_sub(1);
        return;
    }
    if b.skip > 0 {
        return;
    }
    match n {
        "i" | "em" | "cite" | "var" | "dfn" => b.italic = b.italic.saturating_sub(1),
        "b" | "strong" => b.bold = b.bold.saturating_sub(1),
        "sup" => b.sup = b.sup.saturating_sub(1),
        "a" => {
            if let Some((href, start)) = b.open_links.pop()
                && start != u32::MAX
                && !href.is_empty()
            {
                b.close_link_at(href, start);
            }
        }
        _ if is_block(n) => {
            b.flush();
            if let Some(i) = b.stack.iter().rposition(|s| s == n) {
                b.stack.truncate(i);
            }
            match n {
                "ul" | "ol" => {
                    b.lists.pop();
                }
                "pre" => b.pre = b.pre.saturating_sub(1),
                _ => {}
            }
        }
        _ => {}
    }
}

/// The contents of an EPUB 3 nav document: (title, level, href).
fn parse_nav(x: &str, dir: &str) -> Vec<(String, u8, String)> {
    let mut out = Vec::new();
    let mut reader = quick_xml::Reader::from_str(x);
    reader.config_mut().check_end_names = false;
    let mut in_toc = false;
    let mut ol = 0u8;
    let mut href: Option<String> = None;
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let n = local(&e);
                let a = attrs(&e);
                if n == "nav" {
                    let kind = attr(&a, "epub:type").or_else(|| attr(&a, "type")).unwrap_or_default();
                    if kind.split_whitespace().any(|k| k == "toc") {
                        in_toc = true;
                    }
                } else if in_toc && n == "ol" {
                    ol += 1;
                } else if in_toc && n == "a" {
                    href = attr(&a, "href");
                    text.clear();
                }
            }
            Ok(Event::Text(t)) if href.is_some() => text.push_str(&t.decode().unwrap_or_default()),
            Ok(Event::GeneralRef(r)) if href.is_some() => text.push_str(&entity(&r.decode().unwrap_or_default())),
            Ok(Event::End(e)) => {
                let n = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                if n == "a" {
                    if let Some(h) = href.take() {
                        out.push((collapse(&text), ol.max(1), join(dir, &h)));
                    }
                } else if n == "ol" && in_toc {
                    ol = ol.saturating_sub(1);
                } else if n == "nav" && in_toc {
                    break;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// The contents of an EPUB 2 NCX: (title, level, href).
fn parse_ncx(x: &str, dir: &str) -> Vec<(String, u8, String)> {
    let mut out = Vec::new();
    let mut reader = quick_xml::Reader::from_str(x);
    let mut depth = 0u8;
    let mut in_text = false;
    let mut title = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match local(&e).as_str() {
                "navPoint" | "navpoint" => {
                    depth += 1;
                    title.clear();
                }
                "text" => in_text = true,
                _ => {}
            },
            Ok(Event::Empty(e)) => {
                let n = local(&e);
                if n == "content" && depth > 0 {
                    let a = attrs(&e);
                    if let Some(src) = attr(&a, "src") {
                        out.push((collapse(&title), depth, join(dir, &src)));
                    }
                }
            }
            Ok(Event::Text(t)) if in_text => title.push_str(&t.decode().unwrap_or_default()),
            Ok(Event::GeneralRef(r)) if in_text => title.push_str(&entity(&r.decode().unwrap_or_default())),
            Ok(Event::End(e)) => match String::from_utf8_lossy(e.local_name().as_ref()).as_ref() {
                "navPoint" | "navpoint" => depth = depth.saturating_sub(1),
                "text" => in_text = false,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

// XML helpers

fn local(e: &BytesStart) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase()
}

/// Attributes as (name with any prefix, value unescaped).
fn attrs(e: &BytesStart) -> Vec<(String, String)> {
    e.attributes().flatten().map(|a| (String::from_utf8_lossy(a.key.as_ref()).to_ascii_lowercase(), unescape(&String::from_utf8_lossy(&a.value)))).collect()
}

fn attr(a: &[(String, String)], name: &str) -> Option<String> {
    a.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
}

/// Every tag's name and attributes, in order (for small files like container.xml).
fn each_tag(x: &str, mut f: impl FnMut(&str, &[(String, String)])) {
    let mut reader = quick_xml::Reader::from_str(x);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => f(&local(&e), &attrs(&e)),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

/// A named or numbered entity's text (HTML's common ones; XHTML books use few).
fn entity(name: &str) -> String {
    if let Some(n) = name.strip_prefix('#') {
        let c = if let Some(h) = n.strip_prefix('x').or_else(|| n.strip_prefix('X')) { u32::from_str_radix(h, 16).ok() } else { n.parse().ok() };
        return c.and_then(char::from_u32).map(String::from).unwrap_or_default();
    }
    let c = match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{a0}",
        "mdash" => "—",
        "ndash" => "–",
        "hellip" => "…",
        "lsquo" => "‘",
        "rsquo" => "’",
        "ldquo" => "“",
        "rdquo" => "”",
        "copy" => "©",
        "shy" => "",
        "thinsp" | "ensp" | "emsp" => " ",
        "eacute" => "é",
        "egrave" => "è",
        "aacute" => "á",
        "agrave" => "à",
        "ouml" => "ö",
        "uuml" => "ü",
        "auml" => "ä",
        "ccedil" => "ç",
        "middot" => "·",
        "deg" => "°",
        "times" => "×",
        "para" => "¶",
        "sect" => "§",
        "dagger" => "†",
        "Dagger" => "‡",
        _ => "",
    };
    c.to_string()
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        match rest[i..].find(';') {
            Some(j) if j < 12 => {
                out.push_str(&entity(&rest[i + 1..i + j]));
                rest = &rest[i + j + 1..];
            }
            _ => {
                out.push('&');
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parent(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..i].to_string(),
        None => String::new(),
    }
}

/// `href` relative to the folder `dir` (both inside the zip), with "." and ".." worked out.
fn join(dir: &str, href: &str) -> String {
    let (path, frag) = match href.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (href, None),
    };
    let mut parts: Vec<&str> = if path.starts_with('/') || dir.is_empty() { Vec::new() } else { dir.split('/').filter(|p| !p.is_empty()).collect() };
    if path.is_empty() {
        // "#id": this file; the caller adds its own name.
        return href.to_string();
    }
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    let joined = parts.join("/");
    match frag {
        Some(f) => format!("{joined}#{f}"),
        None => joined,
    }
}

fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_epub(dir: &Path, files: &[(&str, &str)]) -> std::path::PathBuf {
        use std::io::Write;
        let path = dir.join("t.epub");
        let f = std::fs::File::create(&path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, body) in files {
            z.start_file(*name, opts).unwrap();
            z.write_all(body.as_bytes()).unwrap();
        }
        z.finish().unwrap();
        path
    }

    fn sample(dir: &Path) -> std::path::PathBuf {
        write_epub(
            dir,
            &[
                ("mimetype", "application/epub+zip"),
                ("META-INF/container.xml", r#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#),
                (
                    "OEBPS/content.opf",
                    r#"<package xmlns="http://www.idpf.org/2007/opf" xmlns:dc="http://purl.org/dc/elements/1.1/"><metadata><dc:title>Thinking &amp; Doing</dc:title><dc:creator>A. Writer</dc:creator><meta name="cover" content="c"/></metadata>
                    <manifest><item id="c" href="img/cover.png" media-type="image/png"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"/></manifest>
                    <spine><itemref idref="one"/><itemref idref="two"/></spine></package>"#,
                ),
                ("OEBPS/img/cover.png", "not really a png"),
                (
                    "OEBPS/nav.xhtml",
                    r##"<html xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="text/one.xhtml">One</a><ol><li><a href="text/one.xhtml#later">Later on</a></li></ol></li><li><a href="text/two.xhtml">Two</a></li></ol></nav></body></html>"##,
                ),
                (
                    "OEBPS/text/one.xhtml",
                    r##"<html><head><title>x</title><style>p{}</style></head><body><h1>Chapter   one</h1>
                    <p>Fast and <i>slow</i>, said <b>he</b>.<a href="two.xhtml#n1" epub:type="noteref"><sup>1</sup></a></p>
                    <blockquote><p>A quote&nbsp;here</p></blockquote><ul><li>first</li><li>second</li></ul><ol><li>uno</li></ol>
                    <p id="later">Line<br/>break</p><img src="../img/fig.png" alt="A figure"/></body></html>"##,
                ),
                ("OEBPS/text/two.xhtml", r#"<html><body><h2>Two</h2><p>Text.</p><p id="n1">1. The note.</p></body></html>"#),
            ],
        )
    }

    #[test]
    fn an_epub_reads_into_blocks_with_styles_links_and_contents() {
        let dir = std::env::temp_dir().join(format!("leaf-epub-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = sample(&dir);
        let m = meta(&path).unwrap();
        assert_eq!(m.title, "Thinking & Doing");
        assert_eq!(m.author, "A. Writer");
        assert_eq!(m.cover.as_deref(), Some(&b"not really a png"[..]));
        let d = open(&path).unwrap();
        assert_eq!(d.sections.len(), 2);
        let s = &d.sections[0].blocks;
        assert_eq!(s[0].kind, Kind::Heading(1));
        assert_eq!(s[0].text, "Chapter one");
        assert_eq!(s[1].text, "Fast and slow, said he.1");
        let i = s[1].text.find("slow").unwrap() as u32;
        assert_eq!(s[1].style_at(i), ITALIC);
        assert_eq!(s[1].style_at(s[1].text.find("he.").unwrap() as u32), BOLD);
        assert_eq!(s[1].links.len(), 1);
        assert_eq!(s[1].links[0].target, Target::Inside(1, 2), "the note's paragraph in the second file");
        assert_eq!(s[2].kind, Kind::Quote);
        assert_eq!(s[2].text, "A quote\u{a0}here");
        assert_eq!(s[3].kind, Kind::Item(1, "•".into()));
        assert_eq!(s[5].kind, Kind::Item(1, "1.".into()));
        assert_eq!(s[6].text, "Line\nbreak");
        assert_eq!(s[7].kind, Kind::Image("OEBPS/img/fig.png".into(), "A figure".into()));
        let titles: Vec<(&str, u8, u32, u32)> = d.toc.iter().map(|t| (t.title.as_str(), t.level, t.section, t.block)).collect();
        assert_eq!(titles, vec![("One", 1, 0, 0), ("Later on", 2, 0, 6), ("Two", 1, 1, 0)]);
        assert_eq!(d.chars, d.sections.iter().flat_map(|s| &s.blocks).map(|b| b.chars()).sum::<u64>());
        let (title, start, end) = d.chapter_span(0, 6);
        assert_eq!(title, "Later on");
        assert!(start < end);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn text_files_split_into_paragraphs_and_headings() {
        let d = parse_txt("CHAPTER I\n\nIt was a\nlong day.\n\n\nThe end.", "t");
        let b = &d.sections[0].blocks;
        assert_eq!(b[0].kind, Kind::Heading(2));
        assert_eq!(b[1].text, "It was a long day.");
        assert_eq!(b[2].text, "The end.");
        assert_eq!(d.toc.len(), 1);
        assert_eq!(d.chars, ("CHAPTER I".len() + "It was a long day.".len() + "The end.".len()) as u64);
    }

    #[test]
    fn paths_join_inside_the_zip() {
        assert_eq!(join("OEBPS/text", "../img/a.png"), "OEBPS/img/a.png");
        assert_eq!(join("", "a.xhtml#x"), "a.xhtml#x");
        assert_eq!(join("OEBPS", "./b%20c.xhtml"), "OEBPS/b%20c.xhtml");
        assert_eq!(percent_decode("b%20c.xhtml"), "b c.xhtml");
        assert_eq!(unescape("a &amp; b &#x41;"), "a & b A");
        assert_eq!(stem(Path::new("/x/thinking_fast-and-slow.epub")), "thinking fast and slow");
        assert_eq!(tidy_caps("THE SIXTH BOOK"), "The Sixth Book");
        assert_eq!(tidy_caps("BOOK IV. OF THE MIND"), "Book IV. of the Mind");
        assert_eq!(tidy_caps("Already Fine"), "Already Fine");
        assert_eq!(txt_meta("x\nTitle: Meditations\nAuthor: Marcus Aurelius\n"), (Some("Meditations".into()), Some("Marcus Aurelius".into())));
    }
}
