//! The whole app: the server, what it last said, the pages open, input, and one frame at a time.
//!
//! Screens are functions of the app's state that draw themselves and act on taps as they draw
//! (immediate mode, as Tally RS): each module has `pub fn draw(app: &mut App, dt: f32) -> bool`,
//! true while something on it is still moving. Nothing animates at rest: after an input the app
//! draws until scrolling settles, then sleeps until the next input or a worker's wake.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use leaf_proto::{Alternative, Series, SeriesDetail, SourceHealth, Status, Update};

use crate::draw::{Canvas, WIDTH};
use crate::images::Images;
use crate::kit::{self, Focus, Kb};
use crate::logic;
use crate::net::{Done, Job, Jobs, Server};
use crate::ui::{Scroll, Touch, Ui};

/// What the platform does for the app.
pub trait Host: Send + Sync {
    /// Where the settings, the copies of what the server said and the page cache live.
    fn data_dir(&self) -> PathBuf;
    fn log(&self, text: &str);
    /// Show the soft keyboard for a field holding `text`, or hide it.
    fn keyboard(&self, _show: bool, _text: &str, _kind: Kb) {}
    /// Hide the system bars while reading (and bring them back after).
    fn immersive(&self, _on: bool) {}
    /// Ask the person for books (EPUB, PDF, text), several at once. Each file comes back through
    /// [`crate::shelf::deliver`].
    fn pick_books(&self) {}
    /// Put words on the clipboard.
    fn copy_text(&self, _text: &str) {}
    /// Hand a file to another app (Android's share sheet). The desktop saves it instead and
    /// answers where, in words.
    fn share_file(&self, _name: &str, _mime: &str, _bytes: &[u8]) -> Option<String> {
        None
    }
    /// A PDF's page count, where the platform can read PDFs (Android).
    fn pdf_pages(&self, _path: &std::path::Path) -> Option<u32> {
        None
    }
    /// A PDF's page `n` drawn `width` pixels wide, as RGBA: (width, height, pixels).
    fn pdf_page(&self, _path: &std::path::Path, _n: u32, _width: u32) -> Option<(u32, u32, Vec<u8>)> {
        None
    }
    /// Ask to post notifications (Android 13 and later), for a bell just turned on.
    fn ask_notifications(&self) {}
    /// A book or a strip is on screen (or no longer): keep the screen on, and let the volume keys
    /// turn pages when `volume_keys`.
    fn reading(&self, _on: bool, _volume_keys: bool) {}
}

/// When the next frame is wanted.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Next {
    Now,
    OnInput,
    /// Sleep until input or this time on [`crate::clock::now_ns`], whichever comes first.
    At(u64),
}

pub const FONTS: [&[u8]; crate::draw::FACES] = [
    include_bytes!("../assets/fonts/geist_400.ttf"),
    include_bytes!("../assets/fonts/geist_500.ttf"),
    include_bytes!("../assets/fonts/geist_600.ttf"),
    include_bytes!("../assets/fonts/sora_600.ttf"),
    // Literata (OFL, assets/fonts/Literata-OFL.txt): the books' reading face.
    include_bytes!("../assets/fonts/literata_400.ttf"),
    include_bytes!("../assets/fonts/literata_400i.ttf"),
    include_bytes!("../assets/fonts/literata_600.ttf"),
];

/// The most a field holds, in characters.
const MAX_TYPED: usize = 300;

#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    /// The server's address and token: the first run, and Settings.
    Connect,
    /// The first tab: the spotlight, Continue reading, Recently updated.
    Home,
    /// The second tab: comics on the server's sources.
    Search,
    /// The third tab: Recent, Following, Downloads.
    Space,
    Series(i64),
    /// The reader, over the series it was opened from ([`App::reader`]); a PDF too.
    Reader,
    /// A book open ([`App::book`]).
    Book,
    /// Search inside the open book.
    InSearch,
    /// Every highlight, by book.
    Notes,
    /// One book's highlights and notes.
    BookNotes(i64),
    /// The daily review of highlights.
    Review,
    Stats,
    /// Search the library: titles, authors and your notes.
    Find,
    /// Files being brought in.
    Imports,
}

/// Something the server is asked for: what it last said, and whether it is being asked again.
#[derive(Debug)]
pub struct Loaded<T> {
    pub data: Option<T>,
    /// The server answered this time (not only the copy on disk).
    pub live: bool,
    /// The last ask failed, in words.
    pub error: Option<String>,
    pub loading: bool,
}

impl<T> Default for Loaded<T> {
    fn default() -> Self {
        Loaded { data: None, live: false, error: None, loading: false }
    }
}

impl<T> Loaded<T> {
    /// Shown from the copy on disk because the server could not be reached.
    pub fn offline(&self) -> bool {
        self.data.is_some() && !self.live && self.error.is_some()
    }
}

#[derive(Default)]
pub struct States {
    pub connect: crate::connect::State,
    pub search: crate::search::State,
    pub series: crate::series::State,
    pub home: crate::home::State,
    pub space: crate::space::State,
    pub notes: crate::notes::State,
    pub review: crate::notes::Review,
    pub find: crate::notes::Find,
}

/// The settings kept in `config.json` (Android's chapter notices read `server` from it too).
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
struct Config {
    server: Option<Server>,
    /// Series whose chapters are listed oldest first.
    #[serde(default)]
    oldest_first: Vec<i64>,
    /// How Following is sorted ([`crate::logic::FOLLOWING_SORTS`]).
    #[serde(default)]
    following_sort: u8,
    /// Notifications were asked for once (the first bell turned on).
    #[serde(default)]
    asked_notifications: bool,
    #[serde(default)]
    reading: ReadPrefs,
    /// Read by Android's chapter job (`LeafChaptersJob`) too.
    #[serde(default)]
    notify: crate::notices::Notify,
    /// Updates shows every series in My space, not only belled ones.
    #[serde(default)]
    updates_all: bool,
    #[serde(default)]
    backup: Option<BackupInfo>,
}

/// How reading feels, kept in `config.json`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ReadPrefs {
    /// A tap in the top or bottom third of a page turns it (the middle shows the controls).
    pub tap_zones: bool,
    /// The volume keys turn pages while reading (Android).
    pub volume_keys: bool,
    /// Blank white or black runs at a comic page's top and bottom are cut short.
    pub trim_gutters: bool,
    /// The dimmer: a black veil over the page this opaque, 0 when off.
    pub dim: f32,
    /// Auto-scroll's speed per series (1 to [`logic::SPEEDS`]).
    pub speeds: std::collections::BTreeMap<i64, u8>,
}

impl Default for ReadPrefs {
    fn default() -> ReadPrefs {
        ReadPrefs { tap_zones: true, volume_keys: true, trim_gutters: true, dim: 0.0, speeds: Default::default() }
    }
}

/// The dimmer goes no darker than this.
pub const DIM_MAX: f32 = 0.85;
/// A tap in a third, a volume key: the page moves this much of the screen.
pub const PAGE_TURN: f32 = 0.85;

/// The last backup exported: its file name, and when the server made it (ms; 0 if unknown).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct BackupInfo {
    pub name: String,
    pub made_ms: i64,
}

pub struct App {
    pub canvas: Canvas,
    pub ui: Ui,
    pub host: Arc<dyn Host>,
    pub focus: Focus,
    /// The pages open, the current one last, each with its scroll.
    pub nav: Vec<(Route, Scroll)>,
    /// True on the first frame a page shows (opened, or uncovered by Back).
    pub fresh: bool,
    pub server: Option<Server>,
    pub data: PathBuf,
    pub jobs: Jobs,
    pub images: Images,
    pub library: Loaded<Vec<Series>>,
    pub series: HashMap<i64, Loaded<SeriesDetail>>,
    pub st: States,
    pub reader: Option<crate::reader::Reader>,
    /// The books on the phone.
    pub shelf: crate::shelf::Shelf,
    pub book: Option<crate::book::BookReader>,
    /// Books read lately, kept so opening one again is instant.
    pub docs: HashMap<i64, Arc<crate::epub::Doc>>,
    pub reading_type: crate::typeset::Type,
    /// Series listed oldest first (Newest ⇅ on the series page).
    pub oldest_first: HashSet<i64>,
    pub following_sort: u8,
    asked_notifications: bool,
    pub prefs: ReadPrefs,
    pub notify: crate::notices::Notify,
    pub updates_all: bool,
    pub backup: Option<BackupInfo>,
    /// Chapters that came for series in My space (`/v1/updates`).
    pub updates: Loaded<Vec<Update>>,
    /// The server's status, as Settings shows it (AniList).
    pub status: Loaded<Status>,
    pub health: Loaded<Vec<SourceHealth>>,
    /// "Wrong series?": the other entries for a series' work.
    pub alternatives: HashMap<i64, Loaded<Vec<Alternative>>>,
    /// A chapter to open once its series has loaded (an update tapped): (series, chapter).
    pub open_after: Option<(i64, i64)>,
    /// What chapters' read marks were before "Mark all before as read", per series, until the
    /// server agrees.
    read_undo: HashMap<i64, Vec<(i64, bool)>>,
    /// A series' shelf before it was moved, until the server agrees.
    shelf_undo: HashMap<i64, Option<String>>,
    /// A backup being exported.
    pub backing_up: bool,
    /// The dimmer moved: kept once the finger lifts.
    pub dim_dirty: bool,
    /// Words a thread of the app's own left for the toast (the share sheet's answer).
    late_toast: Arc<std::sync::Mutex<Option<String>>>,
    /// Chapters asked to download on the server and not yet downloaded there.
    pub downloading: HashSet<i64>,
    /// A short message at the bottom, until this time.
    toast: Option<(String, u64)>,
    /// A field just let go of its keyboard with Done: its page acts on it (Search searches).
    pub typed_done: Option<&'static str>,
    /// The soft keyboard covers the bottom of the window (its height is in the bottom inset).
    pub keyboard_up: bool,
    /// Screenshots: nothing is written to the server (no progress reported).
    pub shot: bool,
    size: (f32, f32),
    insets: (f32, f32),
    last_ns: u64,
    pub now_ns: u64,
    wake: Option<u64>,
}

impl App {
    pub fn new(host: Arc<dyn Host>, now_ns: u64) -> App {
        let data = host.data_dir();
        let _ = std::fs::create_dir_all(&data);
        // A few bytes, read once at start.
        let config: Config = std::fs::read(data.join("config.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let images = Images::new(data.clone());
        images.set_trim(config.reading.trim_gutters);
        images.set_server(config.server.clone());
        images.set_host(host.clone());
        images.scan_cache();
        // Books need no server: Home is always the first page.
        let start = Route::Home;
        let mut app = App {
            canvas: Canvas::new(FONTS),
            ui: Ui::default(),
            host,
            focus: Focus::default(),
            nav: vec![(start, Scroll::default())],
            fresh: true,
            server: config.server,
            jobs: Jobs::new(data.clone()),
            images,
            data: data.clone(),
            library: Loaded::default(),
            series: HashMap::new(),
            st: States::default(),
            reader: None,
            shelf: crate::shelf::Shelf::new(data.clone()),
            book: None,
            docs: HashMap::new(),
            reading_type: crate::typeset::Type::default(),
            oldest_first: config.oldest_first.iter().copied().collect(),
            following_sort: config.following_sort.min(2),
            asked_notifications: config.asked_notifications,
            prefs: config.reading.clone(),
            notify: config.notify,
            updates_all: config.updates_all,
            backup: config.backup.clone(),
            updates: Loaded::default(),
            status: Loaded::default(),
            health: Loaded::default(),
            alternatives: HashMap::new(),
            open_after: None,
            read_undo: HashMap::new(),
            shelf_undo: HashMap::new(),
            backing_up: false,
            dim_dirty: false,
            late_toast: Arc::new(std::sync::Mutex::new(None)),
            downloading: HashSet::new(),
            toast: None,
            typed_done: None,
            keyboard_up: false,
            shot: false,
            size: (1080.0, 2340.0),
            insets: (0.0, 0.0),
            last_ns: now_ns,
            now_ns,
            wake: None,
        };
        app.st.connect.load(app.server.as_ref());
        // Files a picker brought in just before the app last stopped are brought in now.
        let inbox = data.join("inbox");
        let _ = std::thread::Builder::new().name("leaf-inbox".into()).spawn(move || {
            for e in std::fs::read_dir(&inbox).into_iter().flatten().flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if !name.ends_with(".part") {
                    // "<n>-<name>": the name the person's file had.
                    let shown = name.split_once('-').map_or(name.clone(), |(_, n)| n.to_string());
                    crate::shelf::deliver(e.path(), shown);
                }
            }
        });
        crate::images::prune_cache(data, Vec::new());
        app
    }

    /// Show a short message at the bottom for a few seconds.
    pub fn toast(&mut self, words: &str) {
        let until = self.now_ns + 2_600_000_000;
        self.toast = Some((words.to_string(), until));
        self.wake_at(until);
    }

    /// Open a book from the shelf at its place.
    pub fn open_book(&mut self, id: i64) {
        let Some(b) = self.shelf.book(id).cloned() else { return };
        if b.kind == crate::shelf::Kind::Pdf {
            if b.pages == 0 || !cfg!(target_os = "android") {
                self.toast("PDFs open on the phone: the desktop can't draw them");
                return;
            }
            self.images.set_pdf(b.id, b.path(&self.data));
            self.reader = Some(crate::reader::Reader::open_pdf(&b));
            self.push(Route::Reader);
            self.host.immersive(true);
            return;
        }
        let ty_ = crate::book::saved_type(self);
        self.reading_type = ty_;
        let doc = self.docs.get(&id).cloned();
        // Only the last few books stay read.
        if self.docs.len() > 3 {
            self.docs.retain(|k, _| *k == id);
        }
        self.book = Some(crate::book::BookReader::open(&b, &self.data, ty_, doc));
        self.push(Route::Book);
        self.host.immersive(true);
    }

    /// Open a series' chapter in the reader, at a page.
    pub fn open_chapter(&mut self, d: &SeriesDetail, index: usize, page: u32) {
        let mut r = crate::reader::Reader::open(d, index, page);
        r.speed = self.speed_of(d.series.id);
        self.reader = Some(r);
        self.host.immersive(true);
        self.push(Route::Reader);
    }

    /// Continue a series where it was left, when its chapters are at hand; else open its page.
    pub fn continue_series(&mut self, id: i64) {
        let d = self.series.get(&id).and_then(|l| l.data.clone());
        match d.as_ref().and_then(|d| logic::continue_point(&d.chapters).map(|c| (d, c))) {
            Some((d, c)) => self.open_chapter(d, c.index, c.page),
            None => self.push(Route::Series(id)),
        }
    }

    /// The app goes to the background (and may be killed there): the open book's place and time,
    /// the open PDF's page and the comic reader's held-back report are kept now.
    pub fn paused(&mut self) {
        if let Some(mut r) = self.book.take() {
            crate::book::close(self, &mut r);
            self.book = Some(r);
        }
        if let Some(mut r) = self.reader.take() {
            if r.local.is_some() {
                self.keep_pdf_place(&r);
            } else if !self.shot
                && let Some((c, p)) = r.flush(self.now_ns)
            {
                self.jobs.send(self.server.as_ref(), Job::Progress(c, p));
            }
            self.reader = Some(r);
        }
    }

    /// A PDF's place, kept on the phone.
    fn keep_pdf_place(&mut self, r: &crate::reader::Reader) {
        let (Some(book), Some((place, progress))) = (r.local, r.local_place()) else { return };
        let now = crate::shelf::now_ms();
        if let Some(b) = self.shelf.book_mut(book) {
            b.place = place;
            b.progress = progress;
            b.opened_ms = now;
        }
        self.shelf.send(crate::shelf::Op::Place { id: book, place, progress, opened_ms: now, chapter: String::new(), chars: 0 });
    }

    fn close_book(&mut self) {
        if let Some(mut r) = self.book.take() {
            crate::book::close(self, &mut r);
            self.host.immersive(false);
        }
    }

    /// The page showing is one of the three tabs (they keep the tab bar).
    pub fn tabbed(&self) -> bool {
        self.nav.len() == 1 && matches!(self.route(), Route::Home | Route::Search | Route::Space)
    }

    /// Go to tab `i`: Home, Search, My space.
    pub fn go_tab(&mut self, i: usize) {
        self.reset_to([Route::Home, Route::Search, Route::Space][i.min(2)].clone());
    }

    pub fn route(&self) -> &Route {
        &self.nav.last().expect("a page is always open").0
    }

    pub fn scroll(&mut self) -> &mut Scroll {
        &mut self.nav.last_mut().expect("a page is always open").1
    }

    /// The window changed size. Insets are the system bars (and the keyboard), in pixels.
    pub fn resize(&mut self, w: f32, h: f32, inset_top: f32, inset_bottom: f32) {
        self.size = (w, h);
        self.insets = (inset_top, inset_bottom);
    }

    pub fn touch(&mut self, t: Touch) {
        let u = self.canvas.u.max(0.01);
        let x0 = ((self.size.0 - WIDTH * u) * 0.5).max(0.0);
        self.ui.touch(t, (t.x - x0) / u, t.y / u);
    }

    /// The mouse wheel (desktop), in pixels down.
    pub fn wheel(&mut self, dy_px: f32) {
        let units = dy_px / self.canvas.u.max(0.01);
        if self.route() == &Route::Book {
            let h = self.canvas.h;
            if let Some(r) = &mut self.book {
                r.scroll_by(&mut self.canvas, &self.images, units, h);
            }
            return;
        }
        if self.route() == &Route::Reader {
            let h = self.canvas.h;
            if let Some(r) = &mut self.reader {
                r.scroll_by(&self.images, units, h);
            }
            return;
        }
        let s = self.scroll();
        s.off = (s.off + units).clamp(0.0, s.max.max(0.0));
    }

    // The server

    pub fn server_label(&self) -> String {
        self.server.as_ref().map(|s| logic::server_label(&s.base)).unwrap_or_else(|| "no server".into())
    }

    /// Use this server from now on (Connect worked), and keep it.
    pub fn set_server(&mut self, s: Server) {
        self.server = Some(s.clone());
        self.images.set_server(Some(s));
        self.images.retry_failed();
        self.save_config();
        // What came from another server is not this one's.
        self.library = Loaded::default();
        self.series.clear();
        self.updates = Loaded::default();
        self.status = Loaded::default();
        self.health = Loaded::default();
        self.alternatives.clear();
    }

    /// Keep the settings, on a thread of its own (a small file, but still a write). Saves land in
    /// the order they were asked: one that finds a later one already written leaves it be.
    pub fn save_config(&self) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static ASKED: AtomicU64 = AtomicU64::new(0);
        static WRITTEN: std::sync::Mutex<u64> = std::sync::Mutex::new(0);
        let mut oldest: Vec<i64> = self.oldest_first.iter().copied().collect();
        oldest.sort_unstable();
        let config = Config {
            server: self.server.clone(),
            oldest_first: oldest,
            following_sort: self.following_sort,
            asked_notifications: self.asked_notifications,
            reading: self.prefs.clone(),
            notify: self.notify,
            updates_all: self.updates_all,
            backup: self.backup.clone(),
        };
        let (path, host) = (self.data.join("config.json"), self.host.clone());
        let n = ASKED.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = std::thread::Builder::new().name("leaf-config".into()).spawn(move || {
            let mut written = WRITTEN.lock().unwrap_or_else(|e| e.into_inner());
            if *written > n {
                return;
            }
            let bytes = serde_json::to_vec_pretty(&config).unwrap_or_default();
            match crate::net::write_atomic(&path, &bytes) {
                Ok(()) => *written = n,
                Err(e) => host.log(&format!("config: {e}")),
            }
        });
    }

    /// Turn a series' bell on or off: shown at once, told to the server, put back if it says no.
    /// The first bell turned on asks to post notifications.
    pub fn set_bell(&mut self, id: i64, on: bool) {
        self.mark_bell(id, on);
        self.ask(Job::Bell(id, on));
        if on && !self.asked_notifications {
            self.asked_notifications = true;
            self.host.ask_notifications();
            self.save_config();
        }
        self.toast(if on { "You'll hear when new chapters come" } else { "No more notices for this series" });
    }

    fn mark_bell(&mut self, id: i64, on: bool) {
        if let Some(list) = &mut self.library.data
            && let Some(s) = list.iter_mut().find(|s| s.id == id)
        {
            s.bell = on;
        }
        if let Some(d) = self.series.get_mut(&id).and_then(|l| l.data.as_mut()) {
            d.series.bell = on;
        }
    }

    /// Download chapters on the server, or remove them there.
    pub fn download(&mut self, chapters: &[i64], on: bool) {
        for &c in chapters {
            if on {
                self.downloading.insert(c);
            }
            self.ask(Job::Download(c, on));
        }
    }

    /// Newest or oldest first on a series page, kept.
    pub fn flip_order(&mut self, id: i64) {
        if !self.oldest_first.remove(&id) {
            self.oldest_first.insert(id);
        }
        self.save_config();
    }

    pub fn set_following_sort(&mut self, by: u8) {
        self.following_sort = by.min(2);
        self.save_config();
    }

    pub fn ask(&mut self, job: Job) {
        // Without a server there is nothing to ask (books need none).
        if self.server.is_none() && !matches!(job, Job::Status(..)) {
            return;
        }
        // A screenshot changes nothing on the server: what it marks stays on its own copy.
        if self.shot && job.writes() {
            return;
        }
        match &job {
            Job::Library => self.library.loading = true,
            Job::Series(id) => self.series.entry(*id).or_default().loading = true,
            Job::Updates(_) => self.updates.loading = true,
            Job::Health => self.health.loading = true,
            Job::Alternatives(id) => self.alternatives.entry(*id).or_default().loading = true,
            _ => {}
        }
        self.jobs.send(self.server.as_ref(), job);
    }

    /// Take what the workers answered.
    fn take_answers(&mut self) {
        self.shelf.take();
        let picked = crate::shelf::take_delivered();
        if !picked.is_empty() {
            let files = picked.into_iter().map(|(p, n)| (p, n, true)).collect();
            self.shelf.import(files, self.host.clone());
            if self.route() != &Route::Imports && self.route() != &Route::Book {
                self.push(Route::Imports);
            }
        }
        let label = self.server_label();
        for done in self.jobs.take() {
            match done {
                Done::Status(s, r) => {
                    if self.server.as_ref().is_some_and(|mine| mine.base == s.base) {
                        let st = &mut self.status;
                        st.loading = false;
                        match &r {
                            Ok(status) => {
                                st.data = Some(status.clone());
                                st.live = true;
                                st.error = None;
                            }
                            Err(f) => st.error = Some(f.words(&label)),
                        }
                    }
                    // Settings asking again is not Connect's check.
                    if self.st.connect.checking {
                        self.st.connect.answered(s, r.map_err(|f| f.words(&logic::server_label(&self.st.connect.tried))));
                    }
                }
                Done::Library { cached, result } => {
                    let l = &mut self.library;
                    match result {
                        Ok(mut list) => {
                            list.iter_mut().for_each(logic::mend_times);
                            l.data = Some(list);
                            if !cached {
                                l.live = true;
                                l.error = None;
                                l.loading = false;
                            }
                        }
                        Err(f) => {
                            l.loading = false;
                            l.live = false;
                            l.error = Some(f.words(&label));
                        }
                    }
                }
                Done::Series { id, cached, result } => {
                    if let Ok(d) = &result
                        && !cached
                    {
                        for c in &d.chapters {
                            if c.downloaded {
                                self.downloading.remove(&c.id);
                            }
                        }
                    }
                    let l = self.series.entry(id).or_default();
                    match result {
                        Ok(mut d) => {
                            logic::mend_times(&mut d.series);
                            // A move on its way to the server shows already.
                            if self.shelf_undo.contains_key(&id)
                                && let Some(s) = self.library.data.as_ref().and_then(|list| list.iter().find(|s| s.id == id))
                            {
                                d.series.shelf = s.shelf.clone();
                            }
                            l.data = Some(d);
                            if !cached {
                                l.live = true;
                                l.error = None;
                                l.loading = false;
                            }
                        }
                        Err(f) => {
                            l.loading = false;
                            l.live = false;
                            l.error = Some(f.words(&label));
                        }
                    }
                }
                Done::Search(q, r) => self.st.search.answered(q, r.map_err(|f| f.words(&label))),
                Done::Followed(r) => match r {
                    Ok(s) => {
                        self.st.search.followed(&s);
                        let id = s.id;
                        if let Some(list) = &mut self.library.data
                            && !list.iter().any(|x| x.id == id)
                        {
                            list.push(s);
                        }
                        if self.route() == &Route::Search {
                            self.push(Route::Series(id));
                        }
                    }
                    Err(f) => self.st.search.follow_failed(f.words(&label)),
                },
                Done::Unfollowed(id, r) => match r {
                    Ok(()) => {
                        if let Some(list) = &mut self.library.data {
                            list.retain(|s| s.id != id);
                        }
                        self.series.remove(&id);
                        self.st.series.busy = false;
                        if self.route() == &Route::Series(id) {
                            self.pop();
                        }
                    }
                    Err(f) => {
                        self.st.series.busy = false;
                        self.st.series.error = Some(f.words(&label));
                    }
                },
                Done::Progress(chapter, r) => {
                    if let Err(f) = r {
                        self.host.log(&format!("progress of chapter {chapter}: {}", f.words(&label)));
                    }
                }
                Done::PageCount(chapter, r) => {
                    if let Some(rd) = &mut self.reader {
                        rd.page_count(chapter, r.ok());
                    }
                }
                Done::Bell(id, on, r) => {
                    if let Err(f) = r {
                        self.mark_bell(id, !on);
                        self.toast(&f.words(&label));
                    }
                }
                Done::Download(chapter, on, r) => match r {
                    Ok(()) => {}
                    Err(f) => {
                        self.downloading.remove(&chapter);
                        if !on {
                            // Removing was shown at once: it is there after all.
                            self.mark_downloaded(chapter, true);
                        }
                        self.toast(&f.words(&label));
                    }
                },
                Done::Updates { cached, result } => {
                    let l = &mut self.updates;
                    match result {
                        Ok(list) => {
                            l.data = Some(list);
                            if !cached {
                                l.live = true;
                                l.error = None;
                                l.loading = false;
                            }
                        }
                        Err(f) => {
                            l.loading = false;
                            l.live = false;
                            l.error = Some(f.words(&label));
                        }
                    }
                }
                Done::ChapterRead(chapter, read, r) => match r {
                    Ok(()) => self.ask(Job::Library),
                    Err(f) => {
                        self.mark_read(chapter, !read);
                        self.toast(&format!("Not marked: {}", lower_first(&f.words(&label))));
                    }
                },
                Done::ReadBefore(series, r) => {
                    let undo = self.read_undo.remove(&series);
                    match r {
                        Ok(()) => {
                            self.ask(Job::Series(series));
                            self.ask(Job::Library);
                        }
                        Err(f) => {
                            for (c, was) in undo.unwrap_or_default() {
                                self.mark_read(c, was);
                            }
                            self.toast(&format!("Not marked: {}", lower_first(&f.words(&label))));
                        }
                    }
                }
                Done::Shelf(series, r) => {
                    let undo = self.shelf_undo.remove(&series);
                    if let Err(f) = r {
                        if let Some(old) = undo {
                            self.mark_shelf(series, old);
                        }
                        self.toast(&format!("Not moved: {}", lower_first(&f.words(&label))));
                    }
                }
                Done::Health(r) => {
                    let l = &mut self.health;
                    l.loading = false;
                    match r {
                        Ok(list) => {
                            l.data = Some(list);
                            l.live = true;
                            l.error = None;
                        }
                        Err(f) => l.error = Some(f.words(&label)),
                    }
                }
                Done::Backup(r) => {
                    self.backing_up = false;
                    match r {
                        Ok(file) => {
                            let made_ms = logic::backup_made_ms(&file.name);
                            self.backup = Some(BackupInfo { name: file.name.clone(), made_ms });
                            self.save_config();
                            // Written and handed over off this thread: a backup can be megabytes.
                            let (host, out) = (self.host.clone(), self.late_toast.clone());
                            let _ = std::thread::Builder::new().name("leaf-share".into()).spawn(move || {
                                let said = host.share_file(&file.name, &file.mime, &file.bytes);
                                *out.lock().unwrap_or_else(|e| e.into_inner()) = said;
                                crate::wake::wake();
                            });
                        }
                        Err(f) => self.toast(&format!("No backup: {}", lower_first(&f.words(&label)))),
                    }
                }
                Done::Anilist(on, r) => match r {
                    Ok(()) => {
                        if let Some(st) = &mut self.status.data {
                            st.anilist = on;
                        }
                        self.st.connect.anilist.clear();
                        self.toast(if on { "AniList connected. Chapters you finish count there now" } else { "AniList disconnected" });
                    }
                    Err(f) => self.toast(&f.words(&label)),
                },
                Done::Alternatives(id, r) => {
                    let l = self.alternatives.entry(id).or_default();
                    l.loading = false;
                    match r {
                        Ok(list) => {
                            l.data = Some(list);
                            l.live = true;
                            l.error = None;
                        }
                        Err(f) => l.error = Some(f.words(&label)),
                    }
                }
                Done::Repinned(id, r) => {
                    self.st.series.repinning = false;
                    match r {
                        Ok(mut s) => {
                            logic::mend_times(&mut s);
                            if let Some(list) = &mut self.library.data
                                && let Some(old) = list.iter_mut().find(|x| x.id == id)
                            {
                                *old = s;
                            }
                            self.alternatives.remove(&id);
                            self.st.series.alternatives = false;
                            self.ask(Job::Series(id));
                            self.toast("Reading from that entry now. Your place came along");
                        }
                        Err(f) => self.toast(&f.words(&label)),
                    }
                }
            }
        }
    }

    /// An update tapped before its series had loaded: open it now that it has.
    fn open_waiting(&mut self) {
        let Some((series, chapter)) = self.open_after else { return };
        let Some(d) = self.series.get(&series).and_then(|l| l.data.clone()) else {
            if self.series.get(&series).is_some_and(|l| !l.loading && l.error.is_some()) {
                self.open_after = None;
                self.toast("That chapter didn't load");
            }
            return;
        };
        self.open_after = None;
        self.open_update(&d, chapter);
    }

    /// Open the reader at a chapter of a series that is at hand (by its id, else its number).
    pub fn open_update(&mut self, d: &SeriesDetail, chapter: i64) {
        let number = self.updates.data.as_ref().and_then(|l| l.iter().find(|u| u.chapter == chapter)).map(|u| u.number);
        let i = d.chapters.iter().position(|c| c.id == chapter).or_else(|| number.and_then(|n| d.chapters.iter().position(|c| c.number == n)));
        match i {
            Some(i) => {
                let c = &d.chapters[i];
                let page = if c.read { 0 } else { logic::resume_page(c) };
                self.open_chapter(d, i, page);
            }
            None => self.push(Route::Series(d.series.id)),
        }
    }

    /// An update tapped: its chapter in the reader, now or once the series arrives.
    pub fn open_chapter_of(&mut self, series: i64, chapter: i64) {
        if let Some(d) = self.series.get(&series).and_then(|l| l.data.clone()) {
            self.open_update(&d, chapter);
        } else {
            self.open_after = Some((series, chapter));
            self.ask(Job::Series(series));
        }
    }

    // Marks shown at once, put back if the server says no

    fn mark_downloaded(&mut self, chapter: i64, on: bool) {
        for l in self.series.values_mut() {
            if let Some(c) = l.data.as_mut().and_then(|d| d.chapters.iter_mut().find(|c| c.id == chapter)) {
                c.downloaded = on;
            }
        }
    }

    /// Show a chapter read or unread everywhere it is listed.
    pub fn mark_read(&mut self, chapter: i64, read: bool) {
        for l in self.series.values_mut() {
            if let Some(c) = l.data.as_mut().and_then(|d| d.chapters.iter_mut().find(|c| c.id == chapter)) {
                c.read = read;
                if read {
                    c.last_page = 0;
                }
            }
        }
        if let Some(list) = &mut self.updates.data {
            for u in list.iter_mut().filter(|u| u.chapter == chapter) {
                u.read = read;
            }
        }
    }

    /// Mark one chapter read or unread (the long press on its row).
    pub fn set_read(&mut self, chapter: i64, read: bool) {
        self.mark_read(chapter, read);
        self.ask(Job::ChapterRead(chapter, read));
    }

    /// Every chapter of a series numbered below `before`, read.
    pub fn read_before(&mut self, series: i64, before: f64) {
        let Some(d) = self.series.get(&series).and_then(|l| l.data.clone()) else { return };
        let mut undo = Vec::new();
        for c in d.chapters.iter().filter(|c| c.number < before) {
            undo.push((c.id, c.read));
            self.mark_read(c.id, true);
        }
        self.read_undo.insert(series, undo);
        self.ask(Job::ReadBefore(series, before));
    }

    /// Remove a chapter's download, from the server and this phone.
    pub fn remove_download(&mut self, chapter: i64) {
        self.mark_downloaded(chapter, false);
        self.downloading.remove(&chapter);
        self.images.remove_chapters(vec![chapter]);
        self.ask(Job::Download(chapter, false));
    }

    fn mark_shelf(&mut self, id: i64, shelf: Option<String>) {
        if let Some(list) = &mut self.library.data
            && let Some(s) = list.iter_mut().find(|s| s.id == id)
        {
            s.shelf = shelf.clone();
        }
        if let Some(d) = self.series.get_mut(&id).and_then(|l| l.data.as_mut()) {
            d.series.shelf = shelf;
        }
    }

    /// Put a series on a shelf: shown at once, told to the server, put back if it says no.
    pub fn set_shelf(&mut self, id: i64, shelf: logic::Shelf) {
        let old = self.library.data.as_ref().and_then(|l| l.iter().find(|s| s.id == id)).map(|s| s.shelf.clone()).or_else(|| self.series.get(&id).and_then(|l| l.data.as_ref()).map(|d| d.series.shelf.clone()));
        self.shelf_undo.entry(id).or_insert(old.flatten());
        self.mark_shelf(id, Some(shelf.key().to_string()));
        self.ask(Job::Shelf(id, Some(shelf.key().to_string())));
        self.toast(&format!("Moved to {}", shelf.words()));
    }

    /// Auto-scroll's speed for a series.
    pub fn speed_of(&self, series: i64) -> u8 {
        self.prefs.speeds.get(&series).copied().unwrap_or(logic::DEFAULT_SPEED)
    }

    pub fn set_speed(&mut self, series: i64, level: u8) {
        self.prefs.speeds.insert(series, level.clamp(1, logic::SPEEDS));
        self.save_config();
    }

    /// A volume key while reading: the page moves down (or up) most of a screen. False when no
    /// book or strip is showing (the key is the system's then).
    pub fn volume(&mut self, down: bool) -> bool {
        let h = self.canvas.h;
        let by = if down { h * PAGE_TURN } else { -h * PAGE_TURN };
        match self.route() {
            Route::Reader => {
                if let Some(r) = &mut self.reader {
                    r.turn(by);
                }
            }
            Route::Book => {
                if let Some(b) = &mut self.book {
                    b.turn(by);
                }
            }
            _ => return false,
        }
        self.wake_at(self.now_ns);
        true
    }

    // Navigation

    fn shown(&mut self) {
        self.fresh = true;
        self.ui.reset();
        // The bars hide only while a book or a strip is on screen (not over its notes or search).
        let reading = matches!(self.route(), Route::Book | Route::Reader);
        self.host.immersive(reading);
        self.host.reading(reading, self.prefs.volume_keys);
        if self.focus.id.take().is_some() {
            self.host.keyboard(false, "", Kb::Text);
        }
    }

    pub fn push(&mut self, route: Route) {
        if self.route() == &route {
            return;
        }
        self.nav.push((route, Scroll::default()));
        self.shown();
    }

    /// Replace everything open with `route` (Connect's "Open your library").
    pub fn reset_to(&mut self, route: Route) {
        self.close_reader();
        self.close_book();
        self.nav = vec![(route, Scroll::default())];
        self.shown();
    }

    fn close_reader(&mut self) {
        if let Some(mut r) = self.reader.take() {
            self.images.leave_pages();
            if r.local.is_some() {
                self.keep_pdf_place(&r);
                self.host.immersive(false);
                return;
            }
            if !self.shot
                && let Some((c, p)) = r.flush(self.now_ns)
            {
                self.jobs.send(self.server.as_ref(), Job::Progress(c, p));
            }
            self.host.immersive(false);
            // What was read shows as read on the pages under it.
            let id = r.series;
            self.ask(Job::Series(id));
            self.ask(Job::Library);
        }
    }

    /// Close the page showing. False when it was the last one.
    pub fn pop(&mut self) -> bool {
        if self.nav.len() <= 1 {
            return false;
        }
        if self.route() == &Route::Reader {
            self.close_reader();
        }
        if self.route() == &Route::Book {
            self.close_book();
        }
        self.nav.pop();
        self.shown();
        true
    }

    /// Back: typing stops first, then a menu or dialog closes, then the page, then Search and My
    /// space go to Home. False when there was nowhere to go back to.
    pub fn back(&mut self) -> bool {
        if self.focus.id.is_some() {
            self.finish_typing();
            return true;
        }
        if self.route() == &Route::Space && self.st.space.close_over() {
            self.ui.reset();
            return true;
        }
        if self.nav.len() == 1 && matches!(self.route(), Route::Search | Route::Space) {
            self.reset_to(Route::Home);
            return true;
        }
        if matches!(self.route(), Route::Series(_)) && self.st.series.close_over() {
            self.ui.reset();
            return true;
        }
        if self.route() == &Route::Book
            && let Some(h) = self.book.as_mut().and_then(|b| b.take_note())
        {
            self.shelf.update_highlight(h);
        }
        if self.route() == &Route::Book
            && let Some(b) = &mut self.book
            && b.close_over(&mut self.reading_type)
        {
            if let Ok(j) = serde_json::to_string(&self.reading_type) {
                self.shelf.set_setting("type", j);
            }
            self.ui.reset();
            return true;
        }
        self.pop()
    }

    // Typing

    /// Text the keyboard sent: the field's whole text (Android's input state).
    pub fn set_text(&mut self, text: &str) {
        if self.focus.id.is_some() {
            self.focus.text = text.chars().take(MAX_TYPED).collect();
        }
    }

    /// The keyboard's Done: the field lets go and the keyboard goes; its page hears which.
    pub fn finish_typing(&mut self) {
        if let Some(id) = self.focus.id.take() {
            self.focus.at = None;
            self.typed_done = Some(id);
            self.host.keyboard(false, "", Kb::Text);
        }
    }

    /// Done (or Enter) from the platform, between frames.
    pub fn done_typing(&mut self) {
        self.finish_typing();
    }

    /// Characters typed on a hardware keyboard (the desktop).
    pub fn type_text(&mut self, s: &str) {
        if self.focus.id.is_some() && self.focus.text.chars().count() < MAX_TYPED {
            self.focus.text.push_str(s);
        }
    }

    pub fn backspace(&mut self) {
        if self.focus.id.is_some() {
            self.focus.text.pop();
        }
    }

    /// Draw again at `at` even with no input.
    pub fn wake_at(&mut self, at: u64) {
        self.wake = Some(self.wake.map_or(at, |w| w.min(at)));
    }

    pub fn clear_colour(&self) -> [f32; 3] {
        match (self.route(), &self.book) {
            (Route::Book, Some(b)) => crate::book::theme(&b.ty).bg.rgb_f32(),
            _ => crate::theme::BG.rgb_f32(),
        }
    }

    // Pages

    /// Start drawing a page's scrolling content under a fixed top of `top_h` units (under the
    /// status bar): moves it with the finger and returns where its content starts.
    pub fn page_begin(&mut self, dt: f32, top_h: f32) -> Page {
        let bottom = self.canvas.h - self.canvas.inset_bottom;
        let viewport = [0.0, 0.0, WIDTH, bottom];
        let ui = &self.ui;
        let sc = &mut self.nav.last_mut().expect("a page").1;
        let moving = if ui.was_covered() { false } else { sc.update(ui, viewport, dt) };
        self.canvas.dy = -sc.off;
        Page { top: self.canvas.inset_top + top_h, bottom, moving }
    }

    /// Finish the page: `content_bottom` is where its last thing ends (content units).
    pub fn page_end(&mut self, page: &Page, content_bottom: f32) {
        self.canvas.dy = 0.0;
        let keyboard = self.keyboard_up;
        let at = self.focus.at;
        let sc = &mut self.nav.last_mut().expect("a page").1;
        sc.max = content_bottom + 24.0 - page.bottom;
        // Typing: keep the field above the keyboard.
        if keyboard && let Some((y, h)) = at {
            let need = y + h + 16.0 - page.bottom;
            if need > sc.off {
                sc.off = need;
                self.wake_at(self.now_ns);
            }
        }
    }

    /// A band of the ground behind a fixed top, `h` units under the status bar, so what scrolls
    /// under it never shows through.
    pub fn top_band(&mut self, h: f32) {
        let c = &mut self.canvas;
        let bottom = c.inset_top + h;
        c.rect(-1000.0, 0.0, WIDTH + 2000.0, bottom, 0.0, crate::theme::BG);
        c.rect_grad(-1000.0, bottom, WIDTH + 2000.0, 12.0, 0.0, crate::theme::BG, crate::theme::BG.a(0.0));
    }

    /// Draw one frame. Returns when the next is wanted.
    pub fn frame(&mut self, now_ns: u64) -> Next {
        let dt = ((now_ns.saturating_sub(self.last_ns)) as f32 / 1e9).clamp(0.0, 0.05);
        self.last_ns = now_ns;
        self.now_ns = now_ns;
        self.wake = None;
        self.take_answers();
        self.open_waiting();
        let late = self.late_toast.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(words) = late {
            self.toast(&words);
        }
        self.focus.keyboard_up = self.keyboard_up;
        let tapped = self.ui.tap.is_some();
        let (w, h) = self.size;
        self.canvas.begin(w, h, self.insets.0, self.insets.1);
        self.images.set_screen_w(w);
        // A tap on the tab bar (or any fixed chrome drawn after the page) is the chrome's: the
        // page under it never sees it. The tab bar's place is known before it draws.
        let tabs = self.tabbed().then(|| {
            let (y, h) = kit::tabs_area(&self.canvas);
            [-1000.0, y, WIDTH + 2000.0, h]
        });
        self.ui.begin_frame(tabs);
        self.focus.at = None;
        kit::ground(&mut self.canvas);
        let arrived = self.images.drain(&mut self.canvas, now_ns);
        let route = self.route().clone();
        let mut moving = match route {
            Route::Connect => crate::connect::draw(self, dt),
            Route::Home => crate::home::draw(self, dt),
            Route::Search => crate::search::draw(self, dt),
            Route::Space => crate::space::draw(self, dt),
            Route::Series(id) => crate::series::draw(self, id, dt),
            Route::Reader => crate::reader::draw(self, dt),
            Route::Book => crate::book::draw(self, dt),
            Route::InSearch => crate::book::draw_search(self, dt),
            Route::Notes => crate::notes::draw(self, dt),
            Route::BookNotes(id) => crate::notes::draw_book(self, id, dt),
            Route::Review => crate::notes::draw_review(self, dt),
            Route::Stats => crate::stats::draw(self, dt),
            Route::Find => crate::notes::draw_find(self, dt),
            Route::Imports => crate::library::draw_imports(self, dt),
        };
        if let Some((words, until)) = self.toast.clone() {
            if now_ns < until {
                let bottom = self.canvas.h - self.canvas.inset_bottom - if self.tabbed() { kit::TABS_H } else { 0.0 };
                kit::toast(&mut self.canvas, &words, bottom);
                self.wake_at(until);
            } else {
                self.toast = None;
            }
        }
        if self.route() == &route {
            self.fresh = false;
        }
        self.typed_done = None;
        // With no waker (a test, the screenshot), frames keep coming while work is out.
        let working = self.images.pending() || self.jobs.running() > 0 || self.shelf.busy();
        moving |= arrived;
        moving |= working && !crate::wake::has();
        if self.dim_dirty && self.ui.finger().is_none() {
            self.dim_dirty = false;
            self.save_config();
        }
        self.canvas.finish();
        self.ui.end_frame();
        if moving || tapped || self.ui.busy() || self.fresh {
            Next::Now
        } else if let Some(at) = self.wake {
            Next::At(at)
        } else {
            Next::OnInput
        }
    }
}

/// A scrolling page's frame: where its content starts and ends on screen.
pub struct Page {
    pub top: f32,
    pub bottom: f32,
    pub moving: bool,
}

/// "Can't reach your server" from "Can't reach..." as the second half of a sentence.
fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) if !s.starts_with("AniList") => f.to_lowercase().chain(c).collect(),
        _ => s.to_string(),
    }
}
