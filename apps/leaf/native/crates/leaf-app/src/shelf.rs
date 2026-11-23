//! The books on the phone: a local SQLite file (`shelf.db` in the data folder) with each book, its
//! place, its highlights and the time read per day, and the import that copies picked files in.
//!
//! One worker thread owns the connection; the app sends it [`Op`]s and keeps in memory what it
//! loaded at start ([`Shelf`]), changing its copy at once and the file behind it. Nothing here
//! touches the disk on the UI thread. Imports run on threads of their own: they read the file's
//! title, author and cover, copy it into `books/`, and hand the row to the worker, which inserts
//! it (or says it is already there).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, OnceLock};

use rusqlite::{Connection, OptionalExtension, params};

use crate::app::Host;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Epub,
    Pdf,
    Txt,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Epub => "epub",
            Kind::Pdf => "pdf",
            Kind::Txt => "txt",
        }
    }
    fn from_word(s: &str) -> Kind {
        match s {
            "pdf" => Kind::Pdf,
            "txt" => Kind::Txt,
            _ => Kind::Epub,
        }
    }
}

/// A place in a book: section (spine item), block, and a byte inside the block's text. For a PDF,
/// `block` is the page and `byte` how far down it, in thousandths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Place {
    pub section: u32,
    pub block: u32,
    pub byte: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Book {
    pub id: i64,
    pub kind: Kind,
    pub title: String,
    pub author: String,
    /// The file's name inside `books/`.
    pub file: String,
    pub added_ms: i64,
    /// When it was last opened, 0 if never.
    pub opened_ms: i64,
    pub place: Place,
    /// How far through, 0 to 1.
    pub progress: f32,
    /// The reader's own speed in this book, characters a minute (0 until measured).
    pub cpm: f32,
    /// A PDF's page count.
    pub pages: u32,
    pub has_cover: bool,
    /// The chapter the place is in, as the contents name it (for the library).
    pub chapter: String,
    /// Characters in the whole book, once it has been opened (for time left).
    pub chars: i64,
}

impl Book {
    pub fn path(&self, data: &Path) -> PathBuf {
        data.join("books").join(&self.file)
    }
}

/// The four highlight colours, each with the meaning it is named for.
pub const COLOURS: [(&str, u32); 4] = [("Key idea", 0xF5D66B), ("Question", 0x7FB5F5), ("Example", 0x7FD89B), ("Disagree", 0xF59AC0)];

#[derive(Clone, Debug, PartialEq)]
pub struct Highlight {
    pub id: i64,
    pub book: i64,
    pub from: Place,
    /// Where it ends (exclusive), in the same section.
    pub to: Place,
    pub text: String,
    /// 0 to 3, into [`COLOURS`].
    pub colour: u8,
    pub note: String,
    /// The chapter it is in, as the contents named it when it was made.
    pub chapter: String,
    pub created_ms: i64,
    /// When the daily review shows it next.
    pub due_ms: i64,
    pub interval_d: i64,
    /// "Never": out of the review for good.
    pub retired: bool,
}

/// What the worker is asked to do.
pub enum Op {
    Add { import: usize, book: Book, cover: Option<Vec<u8>> },
    Remove(i64),
    Place { id: i64, place: Place, progress: f32, opened_ms: i64, chapter: String, chars: i64 },
    Speed { id: i64, cpm: f32 },
    Time { book: i64, day: i64, secs: i64 },
    AddHighlight(Highlight),
    UpdateHighlight(Highlight),
    RemoveHighlight(i64),
    Reviewed { id: i64, due_ms: i64, interval_d: i64, retired: bool, day: i64 },
    Setting(String, String),
}

pub enum Done {
    Loaded(Box<Loaded>),
    Added(usize, Book),
    Already(usize, String),
    ImportFailed(usize, String),
    /// A file came in and is being read: its name.
    Importing(usize),
    Highlight(Highlight),
    Failed(String),
}

#[derive(Default)]
pub struct Loaded {
    pub books: Vec<Book>,
    pub highlights: Vec<Highlight>,
    /// (day, book, seconds).
    pub time: Vec<(i64, i64, i64)>,
    /// Reviews answered per day.
    pub reviews: HashMap<i64, i64>,
    pub settings: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImportState {
    Waiting,
    Working,
    Added(String),
    Already(String),
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct Import {
    pub name: String,
    pub state: ImportState,
}

pub struct Shelf {
    tx: Sender<Op>,
    done_tx: Sender<Done>,
    rx: Receiver<Done>,
    /// Ops and imports not answered yet.
    pending: Arc<AtomicUsize>,
    data: PathBuf,
    pub loaded: bool,
    pub books: Vec<Book>,
    pub highlights: Vec<Highlight>,
    pub time: Vec<(i64, i64, i64)>,
    pub reviews: HashMap<i64, i64>,
    pub settings: HashMap<String, String>,
    pub imports: Vec<Import>,
    pub error: Option<String>,
}

const SCHEMA: &[&str] = &[
    // 1: the first shelf.
    "CREATE TABLE books(id INTEGER PRIMARY KEY, kind TEXT NOT NULL, title TEXT NOT NULL, author TEXT NOT NULL DEFAULT '',
        file TEXT NOT NULL, added_ms INTEGER NOT NULL, opened_ms INTEGER NOT NULL DEFAULT 0,
        section INTEGER NOT NULL DEFAULT 0, block INTEGER NOT NULL DEFAULT 0, byte INTEGER NOT NULL DEFAULT 0,
        progress REAL NOT NULL DEFAULT 0, cpm REAL NOT NULL DEFAULT 0, pages INTEGER NOT NULL DEFAULT 0, cover INTEGER NOT NULL DEFAULT 0,
        chapter TEXT NOT NULL DEFAULT '', chars INTEGER NOT NULL DEFAULT 0);
     CREATE TABLE highlights(id INTEGER PRIMARY KEY, book INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
        section INTEGER NOT NULL, block INTEGER NOT NULL, start INTEGER NOT NULL, end_section INTEGER NOT NULL,
        end_block INTEGER NOT NULL, end_byte INTEGER NOT NULL, text TEXT NOT NULL, colour INTEGER NOT NULL,
        note TEXT NOT NULL DEFAULT '', chapter TEXT NOT NULL DEFAULT '', created_ms INTEGER NOT NULL, due_ms INTEGER NOT NULL,
        interval_d INTEGER NOT NULL DEFAULT 0, retired INTEGER NOT NULL DEFAULT 0);
     CREATE INDEX highlights_book ON highlights(book);
     CREATE TABLE reading(day INTEGER NOT NULL, book INTEGER NOT NULL, secs INTEGER NOT NULL, PRIMARY KEY(day, book));
     CREATE TABLE reviews(day INTEGER PRIMARY KEY, count INTEGER NOT NULL);
     CREATE TABLE settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);",
];

/// Open (or make) the shelf at `path`, bringing its tables up to date.
pub fn open_db(path: &Path) -> rusqlite::Result<Connection> {
    let c = Connection::open(path)?;
    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;")?;
    let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in SCHEMA.iter().enumerate().skip(v.max(0) as usize) {
        c.execute_batch(&format!("BEGIN; {sql}; PRAGMA user_version = {}; COMMIT;", i + 1))?;
    }
    Ok(c)
}

fn load(c: &Connection) -> rusqlite::Result<Loaded> {
    let mut out = Loaded::default();
    let mut st = c.prepare("SELECT id, kind, title, author, file, added_ms, opened_ms, section, block, byte, progress, cpm, pages, cover, chapter, chars FROM books")?;
    out.books = st
        .query_map([], |r| {
            Ok(Book {
                id: r.get(0)?,
                kind: Kind::from_word(&r.get::<_, String>(1)?),
                title: r.get(2)?,
                author: r.get(3)?,
                file: r.get(4)?,
                added_ms: r.get(5)?,
                opened_ms: r.get(6)?,
                place: Place { section: r.get(7)?, block: r.get(8)?, byte: r.get(9)? },
                progress: r.get::<_, f64>(10)? as f32,
                cpm: r.get::<_, f64>(11)? as f32,
                pages: r.get(12)?,
                has_cover: r.get::<_, i64>(13)? != 0,
                chapter: r.get(14)?,
                chars: r.get(15)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut st = c.prepare("SELECT id, book, section, block, start, end_section, end_block, end_byte, text, colour, note, chapter, created_ms, due_ms, interval_d, retired FROM highlights ORDER BY id")?;
    out.highlights = st
        .query_map([], |r| {
            Ok(Highlight {
                id: r.get(0)?,
                book: r.get(1)?,
                from: Place { section: r.get(2)?, block: r.get(3)?, byte: r.get(4)? },
                to: Place { section: r.get(5)?, block: r.get(6)?, byte: r.get(7)? },
                text: r.get(8)?,
                colour: r.get::<_, i64>(9)?.clamp(0, 3) as u8,
                note: r.get(10)?,
                chapter: r.get(11)?,
                created_ms: r.get(12)?,
                due_ms: r.get(13)?,
                interval_d: r.get(14)?,
                retired: r.get::<_, i64>(15)? != 0,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut st = c.prepare("SELECT day, book, secs FROM reading")?;
    out.time = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
    let mut st = c.prepare("SELECT day, count FROM reviews")?;
    out.reviews = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    let mut st = c.prepare("SELECT key, value FROM settings")?;
    out.settings = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    Ok(out)
}

/// Carry out one op. Answers what the app should hear (a new row's id), if anything.
fn apply(c: &Connection, data: &Path, op: Op) -> rusqlite::Result<Option<Done>> {
    match op {
        Op::Add { import, mut book, cover } => {
            let same: Option<i64> = c
                .prepare_cached("SELECT id FROM books WHERE lower(title) = lower(?1) AND lower(author) = lower(?2) AND kind = ?3")?
                .query_row(params![book.title, book.author, book.kind.word()], |r| r.get(0))
                .optional()?;
            if same.is_some() {
                // The copy just made is not needed.
                let _ = std::fs::remove_file(book.path(data));
                return Ok(Some(Done::Already(import, book.title)));
            }
            c.prepare_cached("INSERT INTO books(kind, title, author, file, added_ms, pages, cover) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?.execute(params![
                book.kind.word(),
                book.title,
                book.author,
                book.file,
                book.added_ms,
                book.pages,
                cover.is_some() as i64
            ])?;
            book.id = c.last_insert_rowid();
            if let Some(bytes) = cover {
                let file = crate::images::CoverOf::Book(book.id).file(data);
                if crate::net::write_atomic(&file, &bytes).is_err() {
                    c.prepare_cached("UPDATE books SET cover = 0 WHERE id = ?1")?.execute(params![book.id])?;
                    book.has_cover = false;
                } else {
                    book.has_cover = true;
                }
            }
            Ok(Some(Done::Added(import, book)))
        }
        Op::Remove(id) => {
            let file: Option<String> = c.prepare_cached("SELECT file FROM books WHERE id = ?1")?.query_row(params![id], |r| r.get(0)).optional()?;
            c.prepare_cached("DELETE FROM books WHERE id = ?1")?.execute(params![id])?;
            if let Some(f) = file {
                let _ = std::fs::remove_file(data.join("books").join(f));
            }
            let _ = std::fs::remove_file(crate::images::CoverOf::Book(id).file(data));
            Ok(None)
        }
        Op::Place { id, place, progress, opened_ms, chapter, chars } => {
            c.prepare_cached("UPDATE books SET section = ?2, block = ?3, byte = ?4, progress = ?5, opened_ms = ?6, chapter = ?7, chars = ?8 WHERE id = ?1")?
                .execute(params![id, place.section, place.block, place.byte, progress as f64, opened_ms, chapter, chars])?;
            Ok(None)
        }
        Op::Speed { id, cpm } => {
            c.prepare_cached("UPDATE books SET cpm = ?2 WHERE id = ?1")?.execute(params![id, cpm as f64])?;
            Ok(None)
        }
        Op::Time { book, day, secs } => {
            c.prepare_cached("INSERT INTO reading(day, book, secs) VALUES (?1, ?2, ?3) ON CONFLICT(day, book) DO UPDATE SET secs = secs + excluded.secs")?
                .execute(params![day, book, secs])?;
            Ok(None)
        }
        Op::AddHighlight(mut h) => {
            c.prepare_cached(
                "INSERT INTO highlights(book, section, block, start, end_section, end_block, end_byte, text, colour, note, chapter, created_ms, due_ms, interval_d, retired)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            )?
            .execute(params![
                h.book,
                h.from.section,
                h.from.block,
                h.from.byte,
                h.to.section,
                h.to.block,
                h.to.byte,
                h.text,
                h.colour as i64,
                h.note,
                h.chapter,
                h.created_ms,
                h.due_ms,
                h.interval_d,
                h.retired as i64
            ])?;
            h.id = c.last_insert_rowid();
            Ok(Some(Done::Highlight(h)))
        }
        Op::UpdateHighlight(h) => {
            c.prepare_cached("UPDATE highlights SET colour = ?2, note = ?3, due_ms = ?4, interval_d = ?5, retired = ?6 WHERE id = ?1")?.execute(params![
                h.id,
                h.colour as i64,
                h.note,
                h.due_ms,
                h.interval_d,
                h.retired as i64
            ])?;
            Ok(None)
        }
        Op::RemoveHighlight(id) => {
            c.prepare_cached("DELETE FROM highlights WHERE id = ?1")?.execute(params![id])?;
            Ok(None)
        }
        Op::Reviewed { id, due_ms, interval_d, retired, day } => {
            c.prepare_cached("UPDATE highlights SET due_ms = ?2, interval_d = ?3, retired = ?4 WHERE id = ?1")?.execute(params![id, due_ms, interval_d, retired as i64])?;
            c.prepare_cached("INSERT INTO reviews(day, count) VALUES (?1, 1) ON CONFLICT(day) DO UPDATE SET count = count + 1")?.execute(params![day])?;
            Ok(None)
        }
        Op::Setting(k, v) => {
            c.prepare_cached("INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value")?.execute(params![k, v])?;
            Ok(None)
        }
    }
}

fn worker(data: PathBuf, rx: Receiver<Op>, tx: Sender<Done>, pending: Arc<AtomicUsize>) {
    let send = |d: Done| {
        let _ = tx.send(d);
        crate::wake::wake();
    };
    let c = match open_db(&data.join("shelf.db")) {
        Ok(c) => c,
        Err(e) => {
            send(Done::Failed(format!("Your books couldn't be opened: {e}")));
            pending.fetch_sub(1, Ordering::SeqCst);
            return;
        }
    };
    match load(&c) {
        Ok(l) => send(Done::Loaded(Box::new(l))),
        Err(e) => send(Done::Failed(format!("Your books couldn't be read: {e}"))),
    }
    pending.fetch_sub(1, Ordering::SeqCst);
    while let Ok(op) = rx.recv() {
        match apply(&c, &data, op) {
            Ok(Some(d)) => send(d),
            Ok(None) => {}
            Err(e) => send(Done::Failed(format!("Couldn't save that: {e}"))),
        }
        pending.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Shelf {
    pub fn new(data: PathBuf) -> Shelf {
        let (tx, op_rx) = channel();
        let (done_tx, rx) = channel();
        let pending = Arc::new(AtomicUsize::new(1));
        let (d, t, p) = (data.clone(), done_tx.clone(), pending.clone());
        let _ = std::fs::create_dir_all(data.join("books"));
        std::thread::Builder::new().name("leaf-shelf".into()).spawn(move || worker(d, op_rx, t, p)).expect("the shelf thread starts");
        Shelf {
            tx,
            done_tx,
            rx,
            pending,
            data,
            loaded: false,
            books: Vec::new(),
            highlights: Vec::new(),
            time: Vec::new(),
            reviews: HashMap::new(),
            settings: HashMap::new(),
            imports: Vec::new(),
            error: None,
        }
    }

    pub fn send(&self, op: Op) {
        self.pending.fetch_add(1, Ordering::SeqCst);
        if self.tx.send(op).is_err() {
            self.pending.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Work out: something is being saved, loaded or imported.
    pub fn busy(&self) -> bool {
        self.pending.load(Ordering::SeqCst) > 0
    }

    pub fn book(&self, id: i64) -> Option<&Book> {
        self.books.iter().find(|b| b.id == id)
    }

    pub fn book_mut(&mut self, id: i64) -> Option<&mut Book> {
        self.books.iter_mut().find(|b| b.id == id)
    }

    /// Take what the workers answered. True when something changed.
    pub fn take(&mut self) -> bool {
        let mut any = false;
        while let Ok(d) = self.rx.try_recv() {
            any = true;
            match d {
                Done::Loaded(l) => {
                    let l = *l;
                    self.books = l.books;
                    self.highlights = l.highlights;
                    self.time = l.time;
                    self.reviews = l.reviews;
                    self.settings = l.settings;
                    self.loaded = true;
                }
                Done::Added(i, b) => {
                    self.set_import(i, ImportState::Added(b.title.clone()));
                    self.books.push(b);
                }
                Done::Already(i, t) => self.set_import(i, ImportState::Already(t)),
                Done::ImportFailed(i, why) => self.set_import(i, ImportState::Failed(why)),
                Done::Importing(i) => self.set_import(i, ImportState::Working),
                Done::Highlight(h) => {
                    // Replace the stand-in kept under id 0 with the same place.
                    match self.highlights.iter_mut().find(|x| x.id == 0 && x.book == h.book && x.from == h.from && x.to == h.to) {
                        Some(x) => {
                            // The colour or note may have changed while it was being saved.
                            let (colour, note) = (x.colour, x.note.clone());
                            *x = h.clone();
                            if (colour, note.as_str()) != (h.colour, h.note.as_str()) {
                                x.colour = colour;
                                x.note = note;
                                let copy = x.clone();
                                self.send(Op::UpdateHighlight(copy));
                            }
                        }
                        None => self.highlights.push(h),
                    }
                }
                Done::Failed(e) => self.error = Some(e),
            }
        }
        any
    }

    fn set_import(&mut self, i: usize, s: ImportState) {
        if let Some(x) = self.imports.get_mut(i) {
            x.state = s;
        }
    }

    /// Bring these files in (each a copy the app may move: an inbox file the picker left, or the
    /// desktop's own path to copy), on a thread of their own.
    pub fn import(&mut self, files: Vec<(PathBuf, String, bool)>, host: Arc<dyn Host>) {
        if files.is_empty() {
            return;
        }
        // A finished queue starts over.
        if self.imports.iter().all(|i| !matches!(i.state, ImportState::Waiting | ImportState::Working)) {
            self.imports.clear();
        }
        let first = self.imports.len();
        for (_, name, _) in &files {
            self.imports.push(Import { name: name.clone(), state: ImportState::Waiting });
        }
        let (tx, ops, pending, data) = (self.done_tx.clone(), self.tx.clone(), self.pending.clone(), self.data.clone());
        pending.fetch_add(files.len(), Ordering::SeqCst);
        let _ = std::thread::Builder::new().name("leaf-import".into()).spawn(move || {
            for (i, (src, name, owned)) in files.into_iter().enumerate() {
                let idx = first + i;
                let _ = tx.send(Done::Importing(idx));
                crate::wake::wake();
                let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| prepare(&data, &src, &name, owned, host.as_ref())));
                match got {
                    Ok(Ok((book, cover))) => {
                        // The worker answers it (and counts it done).
                        if ops.send(Op::Add { import: idx, book, cover }).is_err() {
                            pending.fetch_sub(1, Ordering::SeqCst);
                        }
                    }
                    Ok(Err(why)) => {
                        let _ = tx.send(Done::ImportFailed(idx, why));
                        pending.fetch_sub(1, Ordering::SeqCst);
                    }
                    Err(_) => {
                        let _ = tx.send(Done::ImportFailed(idx, "Leaf couldn't read this file".into()));
                        pending.fetch_sub(1, Ordering::SeqCst);
                    }
                }
                if owned {
                    let _ = std::fs::remove_file(&src);
                }
                crate::wake::wake();
            }
        });
    }

    /// The imports still going.
    pub fn importing(&self) -> usize {
        self.imports.iter().filter(|i| matches!(i.state, ImportState::Waiting | ImportState::Working)).count()
    }

    pub fn setting(&self, key: &str) -> Option<&str> {
        self.settings.get(key).map(String::as_str)
    }

    pub fn set_setting(&mut self, key: &str, value: String) {
        if self.settings.get(key) == Some(&value) {
            return;
        }
        self.settings.insert(key.to_string(), value.clone());
        self.send(Op::Setting(key.to_string(), value));
    }

    pub fn remove_book(&mut self, id: i64) {
        self.books.retain(|b| b.id != id);
        self.highlights.retain(|h| h.book != id);
        self.send(Op::Remove(id));
    }

    pub fn add_time(&mut self, book: i64, day: i64, secs: i64) {
        if secs <= 0 {
            return;
        }
        match self.time.iter_mut().find(|t| t.0 == day && t.1 == book) {
            Some(t) => t.2 += secs,
            None => self.time.push((day, book, secs)),
        }
        self.send(Op::Time { book, day, secs });
    }

    pub fn add_highlight(&mut self, h: Highlight) {
        self.highlights.push(h.clone());
        self.send(Op::AddHighlight(h));
    }

    /// Change a highlight's colour or note (one still being saved changes when it lands).
    pub fn update_highlight(&mut self, h: Highlight) {
        if let Some(x) = self.highlights.iter_mut().find(|x| if h.id != 0 { x.id == h.id } else { x.id == 0 && x.book == h.book && x.from == h.from }) {
            *x = h.clone();
        }
        if h.id != 0 {
            self.send(Op::UpdateHighlight(h));
        }
    }

    pub fn remove_highlight(&mut self, h: &Highlight) {
        self.highlights.retain(|x| !(x.id == h.id && x.book == h.book && x.from == h.from));
        if h.id != 0 {
            self.send(Op::RemoveHighlight(h.id));
        }
    }

    pub fn reviewed(&mut self, id: i64, again: Again, now: i64) {
        let (due_ms, interval_d, retired) = again.next(now);
        if let Some(h) = self.highlights.iter_mut().find(|h| h.id == id) {
            h.due_ms = due_ms;
            h.interval_d = interval_d;
            h.retired = retired;
        }
        let day = day_of(now);
        *self.reviews.entry(day).or_insert(0) += 1;
        self.send(Op::Reviewed { id, due_ms, interval_d, retired, day });
    }
}

/// Read a picked file and copy it into `books/`: the row to insert and its cover's bytes.
fn prepare(data: &Path, src: &Path, name: &str, owned: bool, host: &dyn Host) -> Result<(Book, Option<Vec<u8>>), String> {
    let kind = sniff(src, name)?;
    let books = data.join("books");
    std::fs::create_dir_all(&books).map_err(|e| format!("Couldn't make room for it: {e}"))?;
    let now = now_ms();
    let safe: String = crate::epub::stem(Path::new(name)).chars().filter(|c| c.is_alphanumeric() || *c == ' ').take(40).collect::<String>().trim().replace(' ', "-");
    let file = format!("{now}-{}.{}", if safe.is_empty() { "book" } else { &safe }, kind.word());
    let dest = books.join(&file);
    // Moved when the file is the app's own copy (the picker's inbox), copied otherwise.
    let moved = owned && std::fs::rename(src, &dest).is_ok();
    if !moved {
        std::fs::copy(src, &dest).map_err(|e| format!("Couldn't copy it: {e}"))?;
    }
    let undo = |why: String| {
        let _ = std::fs::remove_file(&dest);
        why
    };
    let mut book = Book {
        id: 0,
        kind,
        title: crate::epub::stem(Path::new(name)),
        author: String::new(),
        file,
        added_ms: now,
        opened_ms: 0,
        place: Place::default(),
        progress: 0.0,
        cpm: 0.0,
        pages: 0,
        has_cover: false,
        chapter: String::new(),
        chars: 0,
    };
    let mut cover = None;
    match kind {
        Kind::Epub => {
            let m = crate::epub::meta(&dest).map_err(undo)?;
            book.title = m.title;
            book.author = m.author;
            // Only a cover that decodes is kept.
            cover = m.cover.filter(|b| image::load_from_memory(b).is_ok());
        }
        Kind::Txt => {
            // Its own "Title:" and "Author:" lines, else the file's name.
            let mut head = vec![0u8; 8192];
            let n = std::fs::File::open(&dest).and_then(|mut f| std::io::Read::read(&mut f, &mut head)).unwrap_or(0);
            let (title, author) = crate::epub::txt_meta(&String::from_utf8_lossy(&head[..n]));
            if let Some(t) = title {
                book.title = t;
            }
            book.author = author.unwrap_or_default();
        }
        Kind::Pdf => {
            book.pages = host.pdf_pages(&dest).unwrap_or(0);
            if let Some((w, h, px)) = host.pdf_page(&dest, 0, crate::images::COVER_W * 2) {
                let mut png = Vec::new();
                if let Some(img) = image::RgbaImage::from_raw(w, h, px)
                    && img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).is_ok()
                {
                    cover = Some(png);
                }
            }
        }
    }
    Ok((book, cover))
}

/// The kind of book a file is, by its first bytes, then its name.
fn sniff(path: &Path, name: &str) -> Result<Kind, String> {
    let mut head = [0u8; 8];
    let n = std::fs::File::open(path).and_then(|mut f| std::io::Read::read(&mut f, &mut head)).map_err(|e| format!("Couldn't read the file: {e}"))?;
    let head = &head[..n];
    if n == 0 {
        return Err("The file is empty, or the phone couldn't read it".into());
    }
    let ext = crate::epub::ext(Path::new(name));
    if head.starts_with(b"PK") {
        return if ext == "epub" || ext.is_empty() || ext == "zip" { Ok(Kind::Epub) } else { Err(format!("Leaf can't read .{ext} files yet")) };
    }
    if head.starts_with(b"%PDF") {
        return Ok(Kind::Pdf);
    }
    match ext.as_str() {
        "txt" | "text" | "md" => Ok(Kind::Txt),
        "mobi" | "azw" | "azw3" | "kfx" => Err("Kindle files can't be opened. Import an EPUB of it instead".into()),
        "" => Err("Leaf can't tell what kind of file this is".into()),
        e => Err(format!("Leaf can't read .{e} files yet")),
    }
}

// Days and the review

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// The local zone's offset from UTC, in seconds, read once.
fn zone_offset() -> i64 {
    static OFF: OnceLock<i64> = OnceLock::new();
    *OFF.get_or_init(|| {
        let t: libc::time_t = (now_ms() / 1000) as libc::time_t;
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        let ok = unsafe { !libc::localtime_r(&t, &mut tm).is_null() };
        if ok { tm.tm_gmtoff as i64 } else { 0 }
    })
}

/// The local day a moment falls on, counted from 1970-01-01.
pub fn day_of(ms: i64) -> i64 {
    (ms / 1000 + zone_offset()).div_euclid(86_400)
}

/// Day of the week, Monday 0.
pub fn weekday(day: i64) -> i64 {
    // 1970-01-01 was a Thursday.
    (day + 3).rem_euclid(7)
}

/// When a review card shows again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Again {
    Sooner,
    Normal,
    Later,
    Never,
}

pub const DAY_MS: i64 = 86_400_000;

impl Again {
    pub const ALL: [Again; 4] = [Again::Sooner, Again::Normal, Again::Later, Again::Never];

    pub fn days(self) -> Option<i64> {
        match self {
            Again::Sooner => Some(3),
            Again::Normal => Some(14),
            Again::Later => Some(60),
            Again::Never => None,
        }
    }

    pub fn words(self) -> (&'static str, &'static str) {
        match self {
            Again::Sooner => ("Sooner", "in 3 days"),
            Again::Normal => ("Normal", "in 2 weeks"),
            Again::Later => ("Later", "in 2 months"),
            Again::Never => ("Never", "retire"),
        }
    }

    /// (due, interval in days, retired) after answering at `now`.
    pub fn next(self, now: i64) -> (i64, i64, bool) {
        match self.days() {
            Some(d) => (now + d * DAY_MS, d, false),
            None => (i64::MAX / 2, 0, true),
        }
    }
}

/// A new highlight first comes back the next day.
pub fn first_due(created: i64) -> i64 {
    created + DAY_MS
}

/// Today's review: highlights due by the end of today, the longest waiting first, at most `cap`.
pub fn due_today(hs: &[Highlight], now: i64, cap: usize) -> Vec<i64> {
    let end_of_day = (day_of(now) + 1) * DAY_MS - zone_offset() * 1000;
    let mut due: Vec<&Highlight> = hs.iter().filter(|h| h.id != 0 && !h.retired && h.due_ms < end_of_day).collect();
    due.sort_by_key(|h| (h.due_ms, h.id));
    due.into_iter().take(cap).map(|h| h.id).collect()
}

/// Days read in a row (a minute or more each), up to today; a day not read yet today doesn't
/// break it. Also the best run ever.
pub fn streak(time: &[(i64, i64, i64)], today: i64) -> (i64, i64) {
    let mut per: HashMap<i64, i64> = HashMap::new();
    for (d, _, s) in time {
        *per.entry(*d).or_insert(0) += s;
    }
    let read = |d: i64| per.get(&d).copied().unwrap_or(0) >= 60;
    let mut d = if read(today) { today } else { today - 1 };
    let mut now = 0;
    while read(d) {
        now += 1;
        d -= 1;
    }
    let mut days: Vec<i64> = per.iter().filter(|(_, s)| **s >= 60).map(|(d, _)| *d).collect();
    days.sort();
    let (mut best, mut run, mut last) = (0, 0, i64::MIN);
    for d in days {
        run = if d == last + 1 { run + 1 } else { 1 };
        best = best.max(run);
        last = d;
    }
    (now, best.max(now))
}

/// Seconds read on each of the seven days of the week holding `today`, Monday first.
pub fn week(time: &[(i64, i64, i64)], today: i64) -> [i64; 7] {
    let monday = today - weekday(today);
    let mut out = [0; 7];
    for (d, _, s) in time {
        if (monday..monday + 7).contains(d) {
            out[(d - monday) as usize] += s;
        }
    }
    out
}

/// "4 h 12 min", "38 min", "under a minute".
pub fn duration(secs: i64) -> String {
    let m = secs / 60;
    if m < 1 {
        "under a minute".into()
    } else if m < 60 {
        format!("{m} min")
    } else if m % 60 == 0 {
        format!("{} h", m / 60)
    } else {
        format!("{} h {} min", m / 60, m % 60)
    }
}

/// Files the platform's picker brought in, waiting for the app's loop: (path, name). Filled from
/// the picker's thread (Android) or the desktop's `LEAF_PICK`.
static INBOX: std::sync::Mutex<Vec<(PathBuf, String)>> = std::sync::Mutex::new(Vec::new());

pub fn deliver(path: PathBuf, name: String) {
    INBOX.lock().unwrap_or_else(|e| e.into_inner()).push((path, name));
    crate::wake::wake();
}

pub fn take_delivered() -> Vec<(PathBuf, String)> {
    std::mem::take(&mut *INBOX.lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hl(id: i64, due: i64, retired: bool) -> Highlight {
        Highlight {
            id,
            book: 1,
            from: Place::default(),
            to: Place::default(),
            text: "t".into(),
            colour: 0,
            note: String::new(),
            chapter: String::new(),
            created_ms: 0,
            due_ms: due,
            interval_d: 0,
            retired,
        }
    }

    #[test]
    fn the_review_spaces_highlights_out() {
        let now = 1_800_000_000_000;
        assert_eq!(Again::Sooner.next(now), (now + 3 * DAY_MS, 3, false));
        assert_eq!(Again::Normal.next(now).1, 14);
        assert_eq!(Again::Later.next(now).1, 60);
        assert!(Again::Never.next(now).2);
        let hs = vec![hl(1, now - DAY_MS, false), hl(2, now + 40 * DAY_MS, false), hl(3, now - 2 * DAY_MS, false), hl(4, now - DAY_MS, true), hl(0, 0, false)];
        assert_eq!(due_today(&hs, now, 10), vec![3, 1], "oldest first; retired and unsaved left out");
        assert_eq!(due_today(&hs, now, 1), vec![3]);
    }

    #[test]
    fn a_streak_counts_days_read_up_to_today() {
        let t = vec![(10, 1, 300), (11, 1, 30), (11, 2, 40), (12, 1, 600), (14, 1, 61), (15, 1, 5)];
        // Day 15 has only 5 seconds: not broken, not counted. 14 alone, then a gap at 13.
        assert_eq!(streak(&t, 15), (1, 3));
        assert_eq!(streak(&t, 12), (3, 3));
        assert_eq!(streak(&t, 20), (0, 3));
        assert_eq!(duration(4 * 3600 + 12 * 60), "4 h 12 min");
        assert_eq!(duration(59), "under a minute");
    }

    #[test]
    fn weeks_start_on_monday() {
        // 2026-11-23 is a Monday: day 20780.
        assert_eq!(weekday(20780), 0);
        let t = vec![(20780, 1, 60), (20786, 1, 120), (20787, 1, 999)];
        assert_eq!(week(&t, 20783), [60, 0, 0, 0, 0, 0, 120]);
    }

    #[test]
    fn the_shelf_keeps_books_highlights_and_time() {
        let dir = std::env::temp_dir().join(format!("leaf-shelf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("books")).unwrap();
        let c = open_db(&dir.join("shelf.db")).unwrap();
        let book = Book {
            id: 0,
            kind: Kind::Epub,
            title: "Meditations".into(),
            author: "Marcus Aurelius".into(),
            file: "1-m.epub".into(),
            added_ms: 5,
            opened_ms: 0,
            place: Place::default(),
            progress: 0.0,
            cpm: 0.0,
            pages: 0,
            has_cover: false,
            chapter: String::new(),
            chars: 0,
        };
        let Some(Done::Added(0, b)) = apply(&c, &dir, Op::Add { import: 0, book: book.clone(), cover: None }).unwrap() else { panic!("added") };
        assert!(matches!(apply(&c, &dir, Op::Add { import: 1, book: book.clone(), cover: None }).unwrap(), Some(Done::Already(1, _))));
        apply(&c, &dir, Op::Place { id: b.id, place: Place { section: 2, block: 3, byte: 4 }, progress: 0.5, opened_ms: 9, chapter: "II".into(), chars: 900 }).unwrap();
        let mut h = hl(0, 100, false);
        h.book = b.id;
        let Some(Done::Highlight(h)) = apply(&c, &dir, Op::AddHighlight(h)).unwrap() else { panic!("highlight") };
        apply(&c, &dir, Op::Time { book: b.id, day: 7, secs: 30 }).unwrap();
        apply(&c, &dir, Op::Time { book: b.id, day: 7, secs: 45 }).unwrap();
        apply(&c, &dir, Op::Reviewed { id: h.id, due_ms: 500, interval_d: 3, retired: false, day: 7 }).unwrap();
        apply(&c, &dir, Op::Setting("type".into(), "{}".into())).unwrap();
        drop(c);
        // Opened again, as at the next start.
        let c = open_db(&dir.join("shelf.db")).unwrap();
        let l = load(&c).unwrap();
        assert_eq!(l.books.len(), 1);
        assert_eq!(l.books[0].place, Place { section: 2, block: 3, byte: 4 });
        assert_eq!(l.books[0].opened_ms, 9);
        assert_eq!((l.books[0].chapter.as_str(), l.books[0].chars), ("II", 900));
        assert_eq!(l.highlights[0].due_ms, 500);
        assert_eq!(l.time, vec![(7, b.id, 75)]);
        assert_eq!(l.reviews.get(&7), Some(&1));
        assert_eq!(l.settings.get("type").map(String::as_str), Some("{}"));
        // Removing the book takes its highlights with it.
        apply(&c, &dir, Op::Remove(b.id)).unwrap();
        let l = load(&c).unwrap();
        assert!(l.books.is_empty() && l.highlights.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
