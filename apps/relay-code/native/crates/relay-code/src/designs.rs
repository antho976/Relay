//! Designs (design 44, E1, E3, E4; D10, D11): the repo's designs newest first, each with a
//! filmstrip of its first page's artboards (E1); a design opened on its artboards, page chips on
//! top, phone-shaped ones two across and wide ones the full width, the whole page one tap away
//! (E3); one artboard full screen, swiped to the next, double-tapped to zoom and dragged (E4).
//!
//! Pages are drawn by the PC's own layout code (`design_paint`). Nothing slow runs in a frame:
//! a page's answer is read and laid out once on this screen's worker thread, and every picture
//! is painted there once for its size; a frame only polls calls, takes what the worker finished
//! and puts finished pictures in the canvas.
//!
//! The photos and pictures image nodes show come from the PC (`design.asset.get`, in pieces) on
//! a second thread, the fetch thread, which decodes them too (`design_paint::pictures`): the
//! open design's first, then the list's. A page is shown at once with tiles where they go; each
//! painted picture is named by which of its page's pictures were here when it was asked for
//! (`Plan::pictures_mark`), so one coming makes the pictures it is on painted again, the old
//! ones shown until the new ones are ready.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use rc_link::{Call, Link};
use relay_client::design::Rect;
use serde_json::{Value, json};

use crate::app::App;
use crate::design_paint::{self as dp, Pix, Plan, pictures};
use crate::draw::{Rgba, Text, WIDTH};
use crate::kit;
use crate::theme::{self as t, ty};
use crate::ui::HScroll;

/// A thumbnail's height on the list, in units.
const THUMB_H: f32 = 150.0;
/// The longest side of a picture painted for the canvas, in pixels: a GPU whose picture atlas
/// stops at 2048 still takes it.
const MAX_SIDE: f32 = 2040.0;
/// How much the phone keeps of painted pictures, to put back in the canvas after it let one go.
const KEEP_BYTES: usize = 48 << 20;
/// Pages kept laid out (a big page is several MB laid out).
const KEEP_PLANS: usize = 4;
/// How long one piece of a picture (at most 3 MiB) may take to come.
const PIECE_WAIT: Duration = Duration::from_secs(60);
/// How many times a picture is fetched in a session (the link dropped, or the phone let it go).
const FETCH_TRIES: u8 = 3;

/// The shots' sample photo (`sample`), a small JPEG.
pub const SAMPLE_PICTURE: &[u8] = include_bytes!("../assets/sample-dunes.jpg");
pub const SAMPLE_SRC: &str = "5a3c0ffee0d1e5a3c0ffee0d1e5a3c0f.jpg";

/// A design as `design.list` gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: i64,
    pub title: String,
    pub rev: i64,
    pub pages: i64,
    pub session: Option<String>,
    pub updated_at: String,
}

/// `design.list`'s designs, the deleted left out, the one changed last first.
pub fn items(v: &Value) -> Vec<Item> {
    let mut out: Vec<Item> = v
        .get("designs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|d| d.get("deleted_at").is_none_or(Value::is_null))
        .filter_map(|d| {
            Some(Item {
                id: d.get("id")?.as_i64()?,
                title: d.get("title").and_then(Value::as_str).unwrap_or("Untitled design").to_string(),
                rev: d.get("rev").and_then(Value::as_i64).unwrap_or(0),
                pages: d.get("pages").and_then(Value::as_i64).unwrap_or(0),
                session: d.get("session").and_then(Value::as_str).map(str::to_string),
                updated_at: d.get("updated_at").and_then(Value::as_str).unwrap_or_default().to_string(),
            })
        })
        .collect();
    out.sort_by_key(|i| std::cmp::Reverse(crate::data::parse_ms(&i.updated_at).unwrap_or(0)));
    out
}

/// A `pages_only` answer's pages: id and name ("Page N" when it has none).
pub fn pages(v: &Value) -> Vec<(String, String)> {
    v.get("nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|n| n.get("kind").and_then(Value::as_str) == Some("page"))
        .enumerate()
        .filter_map(|(i, n)| {
            let id = n.get("id")?.as_str()?.to_string();
            let name = n.get("name").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map_or_else(|| format!("Page {}", i + 1), str::to_string);
            Some((id, name))
        })
        .collect()
}

/// A design's line under its title: "4 pages · by bold-puffin · 5m ago".
pub fn subtitle(i: &Item, now_ms: u64) -> String {
    let mut parts = vec![format!("{} page{}", i.pages, if i.pages == 1 { "" } else { "s" })];
    if let Some(s) = &i.session {
        parts.push(format!("by {s}"));
    }
    match crate::data::ago(&i.updated_at, now_ms).as_str() {
        "" => {}
        "now" => parts.push("now".into()),
        a => parts.push(format!("{a} ago")),
    }
    parts.join(" · ")
}

// The worker

#[derive(Clone, Debug, PartialEq)]
enum Tag {
    Thumb(i64),
    Open(i64, String),
}

/// What a picture shows.
#[derive(Clone, Debug)]
enum What {
    Board(String),
    Page,
    Strip,
    /// Part of the page in page px, of one artboard or of everything.
    Region(Option<String>, Rect),
}

impl What {
    fn tag(&self) -> String {
        match self {
            What::Board(id) => format!("b:{id}"),
            What::Page => "p".into(),
            What::Strip => "s".into(),
            What::Region(root, r) => format!("r:{}:{:.0}:{:.0}:{:.0}:{:.0}", root.as_deref().unwrap_or(""), r.x * 2.0, r.y * 2.0, r.w * 2.0, r.h * 2.0),
        }
    }
}

enum Job {
    Plan { tag: Tag, design: i64, value: Value },
    Paint { key: String, plan: Arc<Plan>, what: What, w: u32, h: u32 },
}

enum Done {
    Plan(Tag, Result<Arc<Plan>, String>),
    Paint(String, Pix),
    /// Painting this picture failed (the painter tripped on the page): it is not asked again.
    Unpainted(String),
    /// A fetch ended, the picture in the keep or not (`pictures::status` says).
    Fetched,
    /// Not painted: a newer part of the same artboard was asked for after it (a pan went on).
    Dropped(String),
}

/// A picture to fetch and decode, `soon` for the open design's.
struct Fetch {
    link: Link,
    design: i64,
    src: String,
    want: (u32, u32),
    soon: bool,
}

/// Read picture `src` of `design` from the PC, every piece, and decode it to cover `want` px.
/// Blocks: the fetch thread only.
fn fetch(link: &Link, design: i64, src: &str, want: (u32, u32)) -> Result<dp::pictures::Picture, pictures::Why> {
    use pictures::Why;
    let mut bytes = Vec::new();
    let mut offset = 0u64;
    loop {
        let v = link
            .call_with("design.asset.get", json!({"design_id": design, "src": src, "offset": offset}), PIECE_WAIT)
            .wait(PIECE_WAIT + Duration::from_secs(5))
            .map_err(|e| if e.code.starts_with("link.") { Why::Link(e.message) } else { Why::Refused(e.message) })?;
        if v.get("size").and_then(Value::as_u64).is_some_and(|n| n > pictures::MAX_FILE as u64) {
            return Err(Why::TooBig);
        }
        let piece = v.get("bytes_b64").and_then(Value::as_str).and_then(pictures::base64_decode).ok_or_else(|| Why::Broken("the PC sent no picture".into()))?;
        bytes.extend_from_slice(&piece);
        if bytes.len() > pictures::MAX_FILE {
            return Err(Why::TooBig);
        }
        match v.get("next").and_then(Value::as_u64) {
            Some(n) if n > offset && !piece.is_empty() => offset = n,
            Some(_) => return Err(Why::Broken("the PC's pieces do not follow on".into())),
            None => break,
        }
    }
    pictures::decode(&bytes, want)
}

/// The fetch thread: one picture at a time, the open design's (`soon`) first. It ends with the
/// worker; what it had not fetched yet may be asked for again.
fn fetch_loop(asks: Receiver<Fetch>, done: Sender<Done>) {
    let mut queue: Vec<Fetch> = Vec::new();
    let take = |queue: &mut Vec<Fetch>, f: Fetch| match queue.iter_mut().find(|q| q.src == f.src) {
        Some(q) => q.soon |= f.soon,
        None => queue.push(f),
    };
    loop {
        if queue.is_empty() {
            match asks.recv() {
                Ok(f) => take(&mut queue, f),
                Err(_) => break,
            }
        }
        while let Ok(f) = asks.try_recv() {
            take(&mut queue, f);
        }
        let f = queue.remove(queue.iter().position(|f| f.soon).unwrap_or(0));
        // Asked twice while it was on its way: it is here now.
        if pictures::status(&f.src) != pictures::Status::Loading {
            if done.send(Done::Fetched).is_err() {
                break;
            }
            continue;
        }
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fetch(&f.link, f.design, &f.src, f.want)))
            .unwrap_or_else(|_| Err(pictures::Why::Broken("the decoder tripped on it".into())));
        pictures::put(&f.src, r);
        if done.send(Done::Fetched).is_err() {
            break;
        }
        crate::wake::wake();
    }
    for f in queue {
        pictures::forget(&f.src);
    }
    while let Ok(f) = asks.try_recv() {
        pictures::forget(&f.src);
    }
}

/// Run a job, a panic in the layout or the painter answered as a failure, so the thread lives on.
fn run_safe(job: Job) -> Done {
    let what = match &job {
        Job::Plan { tag, .. } => Err(tag.clone()),
        Job::Paint { key, .. } => Ok(key.clone()),
    };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(job))).unwrap_or_else(|_| match what {
        Err(tag) => Done::Plan(tag, Err("This page could not be laid out on the phone".into())),
        Ok(key) => Done::Unpainted(key),
    })
}

fn run(job: Job) -> Done {
    match job {
        Job::Plan { tag, design, value } => Done::Plan(tag, dp::plan(design, value).map(Arc::new)),
        Job::Paint { key, plan, what, w, h } => {
            let pix = match what {
                What::Board(id) => dp::board(&plan, &id, w, h),
                What::Page => dp::page(&plan, w, h),
                What::Strip => dp::strip(&plan, w, h, f64::from(h) * 0.08, f64::from(h) * 0.06),
                What::Region(root, r) => dp::region(&plan, root.as_deref(), r, w, h),
            };
            Done::Paint(key, pix)
        }
    }
}

/// The next job to run, in the order asked, and the keys of those skipped: a part of an artboard
/// (a zoomed view, still for a moment) is skipped when a newer part of the same artboard waits
/// behind it, so after a pan only where it stopped is painted.
fn next_job(queue: &mut VecDeque<Job>) -> (Option<Job>, Vec<String>) {
    let mut dropped = Vec::new();
    while let Some(job) = queue.pop_front() {
        let superseded = match &job {
            Job::Paint { what: What::Region(root, _), .. } => queue.iter().any(|q| matches!(q, Job::Paint { what: What::Region(r, _), .. } if r == root)),
            _ => false,
        };
        if !superseded {
            return (Some(job), dropped);
        }
        if let Job::Paint { key, .. } = job {
            dropped.push(key);
        }
    }
    (None, dropped)
}

/// One thread for this screen's slow work, in the order it was asked for. It ends with the state.
struct Worker {
    tx: Sender<Job>,
    fetch: Sender<Fetch>,
    rx: Receiver<Done>,
}

impl Worker {
    fn new() -> Worker {
        let (tx, jobs) = mpsc::channel::<Job>();
        let (fetch, asks) = mpsc::channel::<Fetch>();
        let (done, rx) = mpsc::channel();
        let fetched = done.clone();
        let _ = std::thread::Builder::new().name("designs".into()).spawn(move || {
            let mut queue: VecDeque<Job> = VecDeque::new();
            loop {
                if queue.is_empty() {
                    match jobs.recv() {
                        Ok(j) => queue.push_back(j),
                        Err(_) => break,
                    }
                }
                queue.extend(jobs.try_iter());
                let (job, dropped) = next_job(&mut queue);
                for key in dropped {
                    if done.send(Done::Dropped(key)).is_err() {
                        return;
                    }
                }
                let Some(job) = job else { continue };
                if done.send(run_safe(job)).is_err() {
                    break;
                }
                crate::wake::wake();
            }
        });
        let _ = std::thread::Builder::new().name("design-pictures".into()).spawn(move || fetch_loop(asks, fetched));
        Worker { tx, fetch, rx }
    }
}

// State

#[derive(Default)]
enum Stage {
    #[default]
    Idle,
    Pages(Call),
    Page(Call),
    Planning,
    Ready,
    Empty,
    Failed,
}

#[derive(Default)]
struct Thumb {
    rev: i64,
    stage: Stage,
    /// Kept until its strip is painted.
    plan: Option<Arc<Plan>>,
    /// The last strip painted, shown until a newer one is.
    key: Option<String>,
}

/// One artboard full screen (E4).
#[derive(Default)]
pub struct View {
    pub at: usize,
    pub zoom: f32,
    /// How far the zoomed picture's centre is moved from the area's, in units.
    pan: (f32, f32),
    /// Sideways, while swiping to the next or previous one.
    swipe: f32,
    /// Where the swipe is settling: 0, or a whole width either way.
    settle: Option<f32>,
    last_tap: Option<(u64, (f32, f32))>,
    finger: Option<(f32, f32)>,
}

/// The design open (E3).
#[derive(Default)]
pub struct Open {
    pub id: i64,
    /// The repo it was opened in: the chip moving to another leaves it.
    pub repo: Option<i64>,
    pub title: String,
    pub session: Option<String>,
    pub updated_at: String,
    rev: i64,
    pub pages: Vec<(String, String)>,
    pub page: Option<String>,
    pages_call: Option<Call>,
    page_call: Option<(String, Call)>,
    planning: Option<String>,
    pub plan: Option<Arc<Plan>>,
    chips: HScroll,
    /// The whole page fitted, in place of its artboards.
    pub whole: bool,
    pub view: Option<View>,
    error: Option<String>,
}

#[derive(Default)]
pub struct State {
    repo: Option<i64>,
    pub list: Vec<Item>,
    loaded: bool,
    list_call: Option<Call>,
    error: Option<String>,
    thumbs: HashMap<i64, Thumb>,
    pub open: Option<Open>,
    plans: Vec<Arc<Plan>>,
    pics: Vec<(String, Arc<Pix>)>,
    asked: HashSet<String>,
    /// Pictures the painter failed on.
    failed: HashSet<String>,
    /// The last picture drawn for each box (its name without its pictures' mark), shown while
    /// one with more of its pictures paints.
    shown: HashMap<String, String>,
    /// Each plan's pictures' mark, and the keep's [`pictures::changes`] it was read at.
    marks: HashMap<u64, (u64, u64)>,
    /// Each plan whose pictures were asked for: at which [`pictures::changes`], and whether soon.
    wanted: HashMap<u64, (u64, bool)>,
    /// Fetches sent and not ended.
    fetching: usize,
    /// How many times each picture was fetched, and those asked for soon.
    tries: HashMap<String, u8>,
    soon: HashSet<String>,
    worker: Option<Worker>,
    /// Do the slow work in the frame (screenshots, which draw a few frames with no waker).
    inline: bool,
}

impl State {
    /// Back: the full-screen artboard closes first, then the whole page goes back to artboards.
    pub fn close_over(&mut self) -> bool {
        let Some(o) = self.open.as_mut() else { return false };
        if o.view.take().is_some() {
            return true;
        }
        std::mem::take(&mut o.whole)
    }

    fn submit(&mut self, job: Job) {
        if self.inline {
            let d = run_safe(job);
            self.handle(d);
            return;
        }
        let w = self.worker.get_or_insert_with(Worker::new);
        if let Err(mpsc::SendError(job)) = w.tx.send(job) {
            // The thread is gone (it panicked on a page): start another.
            self.new_worker();
            if let Some(w) = &self.worker {
                let _ = w.tx.send(job);
            }
        }
    }

    /// A new worker and fetch thread; the old fetch thread ends, what it had not fetched
    /// forgotten so it is asked for again.
    fn new_worker(&mut self) {
        self.worker = Some(Worker::new());
        self.fetching = 0;
        self.wanted.clear();
    }

    /// Ask for the pictures `plan` needs that are not here, once per change of the keep; the
    /// open design's `soon`.
    fn want_pictures(&mut self, link: &Link, plan: &Plan, soon: bool) {
        if self.inline || plan.pictures.is_empty() {
            return;
        }
        let now = pictures::changes();
        if self.wanted.get(&plan.serial).is_some_and(|&(at, was_soon)| at == now && (was_soon || !soon)) {
            return;
        }
        if self.wanted.len() > 64 {
            self.wanted.clear();
        }
        self.wanted.insert(plan.serial, (now, soon));
        for w in &plan.pictures {
            let ask = match pictures::status(&w.src) {
                pictures::Status::Absent if !pictures::fetchable(&w.src) => {
                    pictures::put(&w.src, Err(pictures::Why::Unsupported));
                    false
                }
                pictures::Status::Absent => {
                    let tries = self.tries.entry(w.src.clone()).or_default();
                    *tries += 1;
                    if *tries > FETCH_TRIES {
                        continue;
                    }
                    pictures::loading(&w.src);
                    true
                }
                // Asked for the list first, now wanted for the open design: sooner.
                pictures::Status::Loading => soon && !self.soon.contains(&w.src),
                _ => false,
            };
            if !ask {
                continue;
            }
            if soon {
                self.soon.insert(w.src.clone());
            }
            if self.worker.is_none() {
                self.new_worker();
            }
            let f = Fetch { link: link.clone(), design: plan.design, src: w.src.clone(), want: (w.w, w.h), soon };
            let sent = self.worker.as_ref().is_some_and(|wk| wk.fetch.send(f).is_ok());
            if sent {
                self.fetching += 1;
            } else {
                pictures::forget(&w.src);
            }
        }
        // `loading` changed the keep: do not walk this plan again for that.
        self.wanted.insert(plan.serial, (pictures::changes(), soon));
    }

    /// The name of picture `what` of `plan` at `pw`×`ph`: its box's, and which of the page's
    /// pictures it is painted with.
    fn key(&mut self, plan: &Plan, what: &What, pw: u32, ph: u32) -> (String, String) {
        let base = format!("dz{}/{}/{pw}x{ph}", plan.serial, what.tag());
        if plan.pictures.is_empty() {
            return (base.clone(), base);
        }
        let now = pictures::changes();
        let mark = match self.marks.get(&plan.serial) {
            Some(&(at, m)) if at == now => m,
            _ => {
                if self.marks.len() > 64 {
                    self.marks.clear();
                }
                let m = plan.pictures_mark();
                self.marks.insert(plan.serial, (now, m));
                m
            }
        };
        let key = if mark == 0 { base.clone() } else { format!("{base}~{mark:x}") };
        (base, key)
    }

    fn handle(&mut self, d: Done) {
        match d {
            Done::Plan(Tag::Thumb(id), r) => {
                let th = self.thumbs.entry(id).or_default();
                match r {
                    Ok(p) => {
                        th.plan = Some(p.clone());
                        th.stage = Stage::Ready;
                        self.keep_plan(p);
                    }
                    Err(_) => th.stage = Stage::Failed,
                }
            }
            Done::Plan(Tag::Open(id, page), r) => {
                if let Ok(p) = &r {
                    self.keep_plan(p.clone());
                }
                // Its layout is in, whatever page shows now: busy() must not wait on it for ever.
                if let Some(o) = self.open.as_mut().filter(|o| o.id == id && o.planning.as_deref() == Some(page.as_str())) {
                    o.planning = None;
                }
                if let Some(o) = self.open.as_mut().filter(|o| o.id == id && o.page.as_deref() == Some(page.as_str())) {
                    match r {
                        Ok(p) => {
                            if let Some(v) = &mut o.view {
                                v.at = v.at.min(p.boards.len().saturating_sub(1));
                            }
                            o.plan = Some(p);
                            o.error = None;
                        }
                        Err(e) => o.error = Some(e),
                    }
                }
            }
            Done::Fetched => self.fetching = self.fetching.saturating_sub(1),
            Done::Dropped(key) => {
                self.asked.remove(&key);
            }
            Done::Unpainted(key) => {
                // Left in `asked`'s place so it is not painted again; a tile shows instead.
                self.asked.remove(&key);
                self.failed.insert(key);
            }
            Done::Paint(key, pix) => {
                self.asked.remove(&key);
                self.pics.retain(|(k, _)| *k != key);
                self.pics.push((key, Arc::new(pix)));
                let mut total: usize = self.pics.iter().map(|(_, p)| p.data.len()).sum();
                while total > KEEP_BYTES && self.pics.len() > 1 {
                    total -= self.pics.remove(0).1.data.len();
                }
            }
        }
    }

    /// A thumbnail scrolled far away: its layout leaves memory (here and in the plans kept). Its
    /// last strip stays to show when it comes back; with none, it is read again then.
    fn let_go(&mut self, id: i64) {
        let Some(th) = self.thumbs.get_mut(&id) else { return };
        let Some(p) = th.plan.take() else { return };
        if th.key.is_none() && matches!(th.stage, Stage::Ready) {
            th.stage = Stage::Idle;
        }
        let open = self.open.as_ref().and_then(|o| o.plan.as_ref());
        if !open.is_some_and(|o| Arc::ptr_eq(o, &p)) {
            self.plans.retain(|q| !Arc::ptr_eq(q, &p));
        }
    }

    fn keep_plan(&mut self, p: Arc<Plan>) {
        self.plans.retain(|q| !(q.design == p.design && q.page == p.page));
        self.plans.push(p);
        if self.plans.len() > KEEP_PLANS {
            self.plans.remove(0);
        }
    }

    fn cached(&self, design: i64, page: &str, rev: i64) -> Option<Arc<Plan>> {
        self.plans.iter().find(|p| p.design == design && p.page == page && p.rev == rev).cloned()
    }

    /// Something is still on its way (a call or the worker's work).
    fn busy(&self) -> bool {
        self.list_call.is_some()
            || !self.asked.is_empty()
            || self.fetching > 0
            || self.thumbs.values().any(|t| matches!(t.stage, Stage::Pages(_) | Stage::Page(_) | Stage::Planning))
            || self.open.as_ref().is_some_and(|o| o.pages_call.is_some() || o.page_call.is_some() || o.planning.is_some())
    }
}

// Taking answers

fn take(app: &mut App) {
    let link = app.link.clone();
    let st = &mut app.st.designs;
    if let Some(r) = st.list_call.as_ref().and_then(Call::poll) {
        st.list_call = None;
        match r {
            Ok(v) => {
                st.list = items(&v);
                st.loaded = true;
                st.error = None;
            }
            Err(e) => st.error = Some(e.message),
        }
    }
    let mut jobs = Vec::new();
    for (&id, th) in st.thumbs.iter_mut() {
        let answer = match &th.stage {
            Stage::Pages(c) | Stage::Page(c) => c.poll(),
            _ => None,
        };
        let Some(r) = answer else { continue };
        th.stage = match (&th.stage, r) {
            (Stage::Pages(_), Ok(v)) => match pages(&v).first() {
                Some((page, _)) => Stage::Page(link.call("design.get", json!({"design_id": id, "page": page}))),
                None => Stage::Empty,
            },
            (Stage::Page(_), Ok(v)) => {
                jobs.push(Job::Plan { tag: Tag::Thumb(id), design: id, value: v });
                Stage::Planning
            }
            _ => Stage::Failed,
        };
    }
    for j in jobs {
        st.submit(j);
    }
    if let Some(o) = st.open.as_mut()
        && let Some(r) = o.pages_call.as_ref().and_then(Call::poll)
    {
        o.pages_call = None;
        match r {
            Ok(v) => {
                o.pages = pages(&v);
                if let Some(d) = v.get("design") {
                    o.title = d.get("title").and_then(Value::as_str).unwrap_or(&o.title).to_string();
                    o.session = d.get("session").and_then(Value::as_str).map(str::to_string);
                    o.updated_at = d.get("updated_at").and_then(Value::as_str).unwrap_or_default().to_string();
                    o.rev = d.get("rev").and_then(Value::as_i64).unwrap_or(o.rev);
                }
                if o.page.as_ref().is_none_or(|p| !o.pages.iter().any(|(id, _)| id == p)) {
                    o.page = o.pages.first().map(|p| p.0.clone());
                }
                if let Some(p) = o.page.clone() {
                    load_page(st, &link, &p);
                }
            }
            Err(e) => o.error = Some(e.message),
        }
    }
    if let Some(o) = st.open.as_mut()
        && let Some((page, r)) = o.page_call.as_ref().and_then(|(p, c)| Some((p.clone(), c.poll()?)))
    {
        o.page_call = None;
        match r {
            Ok(v) => {
                o.planning = Some(page.clone());
                let id = o.id;
                st.submit(Job::Plan { tag: Tag::Open(id, page), design: id, value: v });
            }
            Err(e) => o.error = Some(e.message),
        }
    }
    let done: Vec<Done> = st.worker.as_ref().map(|w| w.rx.try_iter().collect()).unwrap_or_default();
    for d in done {
        st.handle(d);
    }
}

/// Show `page` of the open design: laid out already, or asked for.
fn load_page(st: &mut State, link: &rc_link::Link, page: &str) {
    let Some(o) = st.open.as_ref() else { return };
    let (id, rev) = (o.id, o.rev);
    let cached = st.cached(id, page, rev);
    let Some(o) = st.open.as_mut() else { return };
    o.page = Some(page.to_string());
    if let Some(p) = cached {
        o.plan = Some(p);
        return;
    }
    // The page shown stays until the new one is laid out, unless it is another page.
    if o.plan.as_ref().is_some_and(|p| p.page != page) {
        o.plan = None;
    }
    o.page_call = Some((page.to_string(), link.call("design.get", json!({"design_id": id, "page": page}))));
}

// Pictures

/// The picture `what` of `plan` in the box (`x`, `y`, `w`, `h`, units): drawn when the canvas
/// holds it, put back from the phone's copy when the canvas let it go, else asked of the worker,
/// the last one drawn for the box shown meanwhile. The page's photos are asked for, `soon` for
/// the open design's. The name of the picture drawn, if one was.
#[allow(clippy::too_many_arguments)]
fn picture(app: &mut App, plan: &Arc<Plan>, what: What, x: f32, y: f32, w: f32, h: f32, radius: f32, soon: bool) -> Option<String> {
    if app.online() && !app.shot {
        app.st.designs.want_pictures(&app.link, plan, soon);
    }
    let (pw, ph) = px_size(w, h, app.canvas.u);
    let (base, key) = app.st.designs.key(plan, &what, pw, ph);
    let drawn = |app: &mut App, key: &str| {
        let st = &mut app.st.designs;
        if st.shown.len() > 512 {
            st.shown.clear();
        }
        st.shown.insert(base.clone(), key.to_string());
        Some(key.to_string())
    };
    if show(app, &key, x, y, w, h, radius) {
        return drawn(app, &key);
    }
    let st = &mut app.st.designs;
    if !st.asked.contains(&key) && !st.failed.contains(&key) && !app.canvas.image_waiting(&key) {
        st.asked.insert(key.clone());
        st.submit(Job::Paint { key: key.clone(), plan: plan.clone(), what, w: pw, h: ph });
        if show(app, &key, x, y, w, h, radius) {
            return drawn(app, &key);
        }
    }
    // While it paints (a photo came), the one drawn before.
    let old = app.st.designs.shown.get(&base).filter(|k| **k != key).cloned()?;
    show(app, &old, x, y, w, h, radius).then_some(old)
}

/// Draw the picture `key` if the canvas or the phone's copy has it.
fn show(app: &mut App, key: &str, x: f32, y: f32, w: f32, h: f32, radius: f32) -> bool {
    if !app.canvas.has_image(key) {
        if app.canvas.image_waiting(key) {
            return false;
        }
        let Some(p) = app.st.designs.pics.iter().find(|(k, _)| k == key).map(|(_, p)| p.clone()) else { return false };
        if !app.canvas.add_image(key, p.w, p.h, p.data.clone()) {
            return false;
        }
    }
    app.canvas.image_cover(key, x, y, w, h, radius, Rgba::WHITE)
}

/// A box of `w`×`h` units in pixels at `u` pixels a unit, its longest side at most [`MAX_SIDE`].
pub fn px_size(w: f32, h: f32, u: f32) -> (u32, u32) {
    let (mut pw, mut ph) = (w * u, h * u);
    let k = (MAX_SIDE / pw.max(ph)).min(1.0);
    pw *= k;
    ph *= k;
    (pw.round().max(1.0) as u32, ph.round().max(1.0) as u32)
}

// E1: the list

fn reload(app: &mut App) {
    let Some(repo) = app.repo else { return };
    if !app.online() || app.shot {
        return;
    }
    let st = &mut app.st.designs;
    if st.list_call.is_none() {
        st.list_call = Some(app.link.call("design.list", json!({"project_id": repo})));
    }
}

fn same_repo(app: &mut App) {
    if app.st.designs.repo != app.repo {
        let st = &mut app.st.designs;
        st.repo = app.repo;
        st.list.clear();
        st.loaded = false;
        st.list_call = None;
        st.error = None;
        st.thumbs.clear();
    }
}

/// Start reading a design's first page for its thumbnail, two designs at a time.
fn want_thumb(app: &mut App, item: &Item) {
    if app.shot || !app.online() {
        return;
    }
    let st = &mut app.st.designs;
    let reading = st.thumbs.values().filter(|t| matches!(t.stage, Stage::Pages(_) | Stage::Page(_) | Stage::Planning)).count();
    let th = st.thumbs.entry(item.id).or_default();
    let stale = th.rev != item.rev && !matches!(th.stage, Stage::Pages(_) | Stage::Page(_) | Stage::Planning);
    if stale {
        th.stage = Stage::Idle;
        th.rev = item.rev;
    }
    if matches!(th.stage, Stage::Idle) && reading < 2 {
        th.rev = item.rev;
        th.stage = Stage::Pages(app.link.call("design.get", json!({"design_id": item.id, "pages_only": true})));
    }
}

/// A design's thumbnail, or a quiet tile until it is painted. True when the picture is drawn.
fn thumb(app: &mut App, id: i64, x: f32, y: f32, w: f32, h: f32) -> bool {
    let (plan, key) = match app.st.designs.thumbs.get(&id) {
        Some(th) => (th.plan.clone(), th.key.clone()),
        None => (None, None),
    };
    if let Some(p) = plan
        && let Some(drawn) = picture(app, &p, What::Strip, x, y, w, h, 12.0, false)
    {
        let (pw, ph) = px_size(w, h, app.canvas.u);
        let (_, current) = app.st.designs.key(&p, &What::Strip, pw, ph);
        let th = app.st.designs.thumbs.entry(id).or_default();
        // The plan goes once its strip is painted with every photo it will get.
        if drawn == current && p.pictures_settled() {
            th.plan = None;
        }
        th.key = Some(drawn);
        return true;
    }
    if let Some(k) = key {
        if show(app, &k, x, y, w, h, 12.0) {
            return true;
        }
        // Let go by the canvas and the phone both: read it again.
        let st = &mut app.st.designs;
        if let Some(th) = st.thumbs.get_mut(&id)
            && th.plan.is_none()
            && matches!(th.stage, Stage::Ready)
            && !st.pics.iter().any(|(pk, _)| *pk == k)
        {
            th.stage = Stage::Idle;
            th.key = None;
        }
    }
    false
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    same_repo(app);
    // Back on the list: a design opens again on its artboards.
    if let Some(o) = app.st.designs.open.as_mut() {
        o.view = None;
        o.whole = false;
    }
    let changed = app.take_changed("design");
    if app.fresh || changed {
        reload(app);
    }
    take(app);
    let page = app.page_begin(dt, crate::home::TOP_H, 0.0);
    let mut y = page.top + 8.0;
    let list = app.st.designs.list.clone();
    let now_ms = rc_link::link::now_ms();
    let mut open = None;
    if list.is_empty() {
        let st = &app.st.designs;
        let (title, body) = if !app.online() && !st.loaded {
            ("Waiting for your PC", "Designs show here once the PC answers.".to_string())
        } else if !st.loaded {
            match &st.error {
                Some(e) => ("Designs didn't load", e.clone()),
                None => ("Reading designs…", String::new()),
            }
        } else {
            ("No designs in this repo", "Designs an agent draws for this repo show here, newest first.".to_string())
        };
        let body = (!body.is_empty()).then_some(body);
        let (h, _) = kit::notice(&mut crate::cx!(app), 32.0, y + 120.0, WIDTH - 64.0, title, body.as_deref(), None);
        y += 120.0 + h;
    }
    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    for item in &list {
        let row_h = THUMB_H + 10.0 + 22.0 + 18.0 + 22.0;
        let on_screen = y + app.canvas.dy;
        if on_screen < page.bottom + 300.0 && on_screen + row_h > page.top - 300.0 {
            want_thumb(app, item);
            if !thumb(app, item.id, x, y, w, THUMB_H) {
                app.canvas.rect(x, y, w, THUMB_H, 12.0, t::S1);
            }
        } else {
            app.st.designs.let_go(item.id);
        }
        let mut cx = crate::cx!(app);
        if cx.pressed(0.0, y, WIDTH, row_h - 12.0) {
            cx.c.rect(x, y, w, THUMB_H, 12.0, t::WASH);
        }
        cx.c.text_fit(&item.title, x, y + THUMB_H + 10.0, w, Text::new(ty::BODY, t::INK).semi());
        cx.c.text_fit(&subtitle(item, now_ms), x, y + THUMB_H + 33.0, w, Text::new(ty::CAPTION, t::INK3));
        if cx.tap(0.0, y, WIDTH, row_h - 12.0) {
            open = Some(item.id);
        }
        y += row_h;
    }
    app.page_end(&page, y);
    crate::home::top(app, "Designs");
    if let Some(id) = open {
        app.push(crate::app::Route::Design(id));
    }
    page.moving || app.st.designs.busy() && !crate::wake::has()
}

// E3: a design on its artboards

fn ensure_open(app: &mut App, id: i64) {
    if app.st.designs.open.as_ref().is_some_and(|o| o.id == id) {
        return;
    }
    let item = app.st.designs.list.iter().find(|i| i.id == id).cloned();
    app.st.designs.open = Some(Open {
        id,
        repo: app.repo,
        title: item.as_ref().map_or_else(|| "Design".into(), |i| i.title.clone()),
        session: item.as_ref().and_then(|i| i.session.clone()),
        updated_at: item.as_ref().map(|i| i.updated_at.clone()).unwrap_or_default(),
        rev: item.as_ref().map_or(0, |i| i.rev),
        ..Open::default()
    });
    reload_open(app);
}

fn reload_open(app: &mut App) {
    if !app.online() || app.shot {
        return;
    }
    let link = app.link.clone();
    if let Some(o) = app.st.designs.open.as_mut()
        && o.pages_call.is_none()
    {
        o.pages_call = Some(link.call("design.get", json!({"design_id": o.id, "pages_only": true})));
    }
}

/// From a design page to the Designs list: back to it when it is under this page, else in place.
fn leave_for_list(app: &mut App) {
    let n = app.nav.len();
    if n >= 2 && app.nav[n - 2].0 == crate::app::Route::Designs {
        app.pop();
    } else if n >= 2 {
        app.pop();
        app.push(crate::app::Route::Designs);
    } else {
        app.reset_to(crate::app::Route::Designs);
    }
}

/// The top bar of a design and of an artboard: a key at the left, a title over a line, and a
/// key at the right. Returns which was tapped.
fn bar(app: &mut App, left: &'static str, title: &str, sub: &str, right: Option<&'static str>) -> (bool, bool) {
    app.top_band(crate::home::TOP_H);
    let y = app.canvas.inset_top + 6.0;
    let mut cx = crate::cx!(app);
    let l = kit::icon_key(&mut cx, 6.0, y, left);
    let r = right.is_some_and(|g| kit::icon_key(&mut cx, WIDTH - 50.0, y, g));
    let end = if right.is_some() { WIDTH - 56.0 } else { WIDTH - t::PAD };
    cx.c.text_fit(title, 56.0, y + 2.0, end - 56.0, Text::new(ty::SECTION, t::INK).semi());
    cx.c.text_fit(sub, 56.0, y + 24.0, end - 56.0, Text::new(ty::CAPTION, t::INK3));
    (l, r)
}

pub fn draw_design(app: &mut App, id: i64, dt: f32) -> bool {
    // Another repo picked on the chip: this design is the old one's, so its list shows instead,
    // the same kind of page (route A).
    if app.st.designs.open.as_ref().is_some_and(|o| o.id == id && o.repo != app.repo) {
        app.st.designs.open = None;
        leave_for_list(app);
        return true;
    }
    ensure_open(app, id);
    let changed = app.take_changed("design");
    if app.fresh || changed {
        reload_open(app);
    }
    take(app);
    if app.st.designs.open.as_ref().is_some_and(|o| o.view.is_some() && o.plan.as_ref().is_some_and(|p| !p.boards.is_empty())) {
        return viewer(app, dt);
    }
    if let Some(o) = app.st.designs.open.as_mut() {
        o.view = None;
    }
    let page = app.page_begin(dt, crate::home::TOP_H, 0.0);
    let mut y = page.top + 4.0;
    let Some(o) = app.st.designs.open.as_ref() else { return false };
    let plan = o.plan.clone();
    let whole = o.whole;
    let error = o.error.clone();
    let chips: Vec<(String, bool, Option<Rgba>)> = o.pages.iter().map(|(pid, name)| (name.clone(), o.page.as_deref() == Some(pid.as_str()), None)).collect();
    let page_ids: Vec<String> = o.pages.iter().map(|p| p.0.clone()).collect();

    // The page chips.
    if chips.len() > 1 {
        let mut hs = app.st.designs.open.as_mut().map(|o| std::mem::take(&mut o.chips)).unwrap_or_default();
        let hit = kit::chip_row(&mut crate::cx!(app), &mut hs, y, &chips, dt);
        if let Some(o) = app.st.designs.open.as_mut() {
            o.chips = hs;
        }
        if let Some(i) = hit
            && !chips[i].1
        {
            let link = app.link.clone();
            load_page(&mut app.st.designs, &link, &page_ids[i]);
        }
        y += 32.0 + 14.0;
    }

    let (x, w) = (t::PAD, WIDTH - 2.0 * t::PAD);
    let mut open_at = None;
    match &plan {
        None => {
            let words = match (&error, app.online()) {
                (Some(e), _) => e.clone(),
                (None, true) => "Reading the page…".into(),
                (None, false) => "This page shows once the PC answers.".into(),
            };
            app.canvas.para(&words, x, y + 10.0, w, Text::new(ty::UI, t::INK3).line(1.45), 4);
            y += 50.0;
        }
        Some(p) if whole || p.boards.is_empty() => {
            if let Some(b) = p.bounds {
                let room = (page.bottom - page.top - 120.0).max(200.0);
                let s = dp::fit(b.w, b.h, f64::from(w), f64::from(room), 4.0) as f32;
                let (iw, ih) = (b.w as f32 * s, b.h as f32 * s);
                let ix = x + (w - iw) / 2.0;
                if picture(app, p, What::Page, ix, y, iw, ih, 8.0, true).is_none() {
                    app.canvas.rect(ix, y, iw, ih, 8.0, t::S1);
                }
                let tap = app.ui.tap_here();
                let mut cx = crate::cx!(app);
                if cx.tap(ix, y, iw, ih)
                    && let Some((tx, ty_)) = tap
                {
                    // The artboard under the finger, full screen.
                    let (px, py) = (b.x + f64::from((tx - ix) / s), b.y + f64::from((ty_ - app.canvas.dy - y) / s));
                    open_at = p.boards.iter().position(|bd| bd.rect.contains(px, py));
                }
                y += ih + 12.0;
            } else {
                app.canvas.text("Nothing on this page yet.", x, y + 10.0, Text::new(ty::UI, t::INK3));
                y += 40.0;
            }
        }
        Some(p) => {
            let sizes: Vec<(f64, f64)> = p.boards.iter().map(|b| (b.rect.w, b.rect.h)).collect();
            let label = 26.0;
            let (cells, total) = dp::arrange(&sizes, w, 14.0, label, 2.6);
            for (i, (b, c)) in p.boards.iter().zip(&cells).enumerate() {
                let (cx_, cy_, cw, ch) = (x + c[0], y + c[1], c[2], c[3]);
                let on_screen = cy_ + app.canvas.dy;
                if on_screen < page.bottom + 200.0 && on_screen + ch > page.top - 200.0 && picture(app, p, What::Board(b.id.clone()), cx_, cy_, cw, ch, 6.0, true).is_none() {
                    app.canvas.rect(cx_, cy_, cw, ch, 6.0, t::S1);
                }
                // The artboard's edge: its ground is often the app's own.
                app.canvas.outline(cx_, cy_, cw, ch, 6.0, 1.0, t::LINE);
                let mut cx = crate::cx!(app);
                if cx.pressed(cx_, cy_, cw, ch + label) {
                    cx.c.rect(cx_, cy_, cw, ch, 6.0, t::WASH);
                }
                let col_w = if dp::is_wide(b.rect.w, b.rect.h) { w } else { (w - 14.0) / 2.0 };
                let lx = if dp::is_wide(b.rect.w, b.rect.h) { x } else { cx_ - (col_w - cw) / 2.0 };
                cx.c.text_fit(&b.name, lx, cy_ + ch + 6.0, col_w, Text::new(ty::CAPTION, t::INK2));
                if cx.tap(cx_, cy_, cw, ch + label) {
                    open_at = Some(i);
                }
            }
            y += total + 14.0;
        }
    }
    if let Some(cap) = plan.as_ref().and_then(|p| p.omitted_now().caption()) {
        y += app.canvas.para(&cap, x, y, w, Text::new(ty::SMALL, t::INK3).line(1.45), 4) + 8.0;
    }
    app.page_end(&page, y);

    let (title, sub) = {
        let o = app.st.designs.open.as_ref().expect("open");
        let now_ms = rc_link::link::now_ms();
        let ago = crate::data::ago(&o.updated_at, now_ms);
        let when = match ago.as_str() {
            "" => String::new(),
            "now" => "changed now".into(),
            a => format!("changed {a} ago"),
        };
        let sub = [o.session.clone().unwrap_or_default(), when].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        (o.title.clone(), sub)
    };
    let key = if whole { "grid" } else { "maximize" };
    let has_page = plan.as_ref().is_some_and(|p| p.bounds.is_some() && !p.boards.is_empty());
    let (back, toggle) = bar(app, "arrow-left", &title, &sub, has_page.then_some(key));
    if let Some(o) = app.st.designs.open.as_mut() {
        if toggle {
            o.whole = !o.whole;
        }
        if let Some(at) = open_at {
            o.view = Some(View { at, zoom: 1.0, ..View::default() });
        }
    }
    if back {
        app.pop();
    }
    page.moving || app.st.designs.busy() && !crate::wake::has()
}

// E4: one artboard full screen

/// The area an artboard is shown in, under the top bar and over the dots (units: x, y, w, h).
fn view_area(app: &App) -> [f32; 4] {
    let top = app.canvas.inset_top + crate::home::TOP_H;
    let bottom = app.canvas.h - app.canvas.inset_bottom - 36.0;
    [0.0, top, WIDTH, (bottom - top).max(100.0)]
}

/// An artboard's box at rest in the area: fitted with a margin, centred.
pub fn rest_box(bw: f64, bh: f64, area: [f32; 4]) -> [f32; 4] {
    let m = 12.0;
    let s = dp::fit(bw, bh, f64::from(area[2] - 2.0 * m), f64::from(area[3] - 2.0 * m), 4.0) as f32;
    let (w, h) = (bw as f32 * s, bh as f32 * s);
    [area[0] + (area[2] - w) / 2.0, area[1] + (area[3] - h) / 2.0, w, h]
}

/// How far a zoomed picture may be moved: until its edge meets the area's.
pub fn pan_limit(w: f32, h: f32, zoom: f32, area: [f32; 4]) -> (f32, f32) {
    (((w * zoom - area[2]) / 2.0).max(0.0), ((h * zoom - area[3]) / 2.0).max(0.0))
}

fn viewer(app: &mut App, dt: f32) -> bool {
    let Some(plan) = app.st.designs.open.as_ref().and_then(|o| o.plan.clone()) else { return false };
    let n = plan.boards.len();
    let area = view_area(app);
    app.canvas.dy = 0.0;
    let mut moving = false;

    // Gestures: a sideways swipe to the next one, a double tap to zoom, a drag to move it zoomed.
    app.ui.claim_h(area);
    let now = app.now_ns;
    let finger = app.ui.finger();
    let dragging = app.ui.drag_from.is_some_and(|(fx, fy)| fx >= area[0] && fy >= area[1] && fy < area[1] + area[3] && !app.ui.hidden((fx, fy)));
    let tap = app.ui.tap_here().filter(|&(tx, ty_)| ty_ >= area[1] && ty_ < area[1] + area[3] && (0.0..WIDTH).contains(&tx));
    if tap.is_some() {
        app.ui.tap = None;
    }
    let (drag_dx, fling_x, horizontal) = (app.ui.drag_dx, app.ui.fling_x, app.ui.horizontal);
    let Some(v) = app.st.designs.open.as_mut().and_then(|o| o.view.as_mut()) else { return false };
    v.at = v.at.min(n - 1);
    let b = plan.boards[v.at].clone();
    let rest = rest_box(b.rect.w, b.rect.h, area);
    let zoomed = v.zoom > 1.01;
    if let Some(p) = tap {
        match v.last_tap {
            Some((at, q)) if now.saturating_sub(at) < 350_000_000 && (p.0 - q.0).abs() < 30.0 && (p.1 - q.1).abs() < 30.0 => {
                v.last_tap = None;
                let c = (area[0] + area[2] / 2.0, area[1] + area[3] / 2.0);
                if zoomed {
                    v.zoom = 1.0;
                    v.pan = (0.0, 0.0);
                } else {
                    let z = 2.5;
                    v.pan = (-(p.0 - c.0) * (z - 1.0), -(p.1 - c.1) * (z - 1.0));
                    v.zoom = z;
                }
            }
            _ => v.last_tap = Some((now, p)),
        }
    }
    if v.zoom > 1.01 {
        if dragging && let (Some(f), Some(last)) = (finger, v.finger) {
            v.pan.0 += f.0 - last.0;
            v.pan.1 += f.1 - last.1;
        }
        v.finger = if dragging { finger } else { None };
        let (lx, ly) = pan_limit(rest[2], rest[3], v.zoom, area);
        v.pan = (v.pan.0.clamp(-lx, lx), v.pan.1.clamp(-ly, ly));
    } else {
        v.finger = None;
        if dragging && horizontal {
            let past_end = (v.swipe + drag_dx > 0.0 && v.at == 0) || (v.swipe + drag_dx < 0.0 && v.at + 1 >= n);
            v.swipe += if past_end { drag_dx * 0.35 } else { drag_dx };
            v.settle = None;
            moving = true;
        }
        if let Some(fv) = fling_x {
            let next = (v.swipe < -WIDTH * 0.25 || fv < -500.0) && v.at + 1 < n;
            let prev = (v.swipe > WIDTH * 0.25 || fv > 500.0) && v.at > 0;
            v.settle = Some(if next { -WIDTH } else if prev { WIDTH } else { 0.0 });
        }
        if let Some(target) = v.settle {
            v.swipe += (target - v.swipe) * (1.0 - (-dt * 16.0).exp());
            if (target - v.swipe).abs() < 0.5 {
                if target < 0.0 {
                    v.at += 1;
                } else if target > 0.0 {
                    v.at -= 1;
                }
                v.swipe = 0.0;
                v.settle = None;
            }
            moving = true;
        } else if !dragging && v.swipe != 0.0 {
            v.settle = Some(0.0);
            moving = true;
        }
    }
    let (at, zoom, pan, swipe, still) = (v.at, v.zoom, v.pan, v.swipe, finger.is_none() && v.settle.is_none());

    // The artboards: this one, and the ones beside it while swiping.
    app.canvas.push_clip(area[0], area[1], area[2], area[3]);
    for k in at.saturating_sub(1)..(at + 2).min(n) {
        let bd = plan.boards[k].clone();
        let r = rest_box(bd.rect.w, bd.rect.h, area);
        let off = swipe + (k as f32 - at as f32) * WIDTH;
        if k != at && swipe == 0.0 {
            // Painted ahead, so a swipe never lands on an empty tile.
            let (pw, ph) = px_size(r[2], r[3], app.canvas.u);
            let (_, key) = app.st.designs.key(&plan, &What::Board(bd.id.clone()), pw, ph);
            if !app.canvas.has_image(&key) && !app.st.designs.pics.iter().any(|(pk, _)| *pk == key) && !app.st.designs.asked.contains(&key) {
                app.st.designs.asked.insert(key.clone());
                app.st.designs.submit(Job::Paint { key, plan: plan.clone(), what: What::Board(bd.id.clone()), w: pw, h: ph });
            }
            continue;
        }
        if k == at && zoom > 1.01 {
            let (w, h) = (r[2] * zoom, r[3] * zoom);
            let (x, y) = (r[0] + r[2] / 2.0 - w / 2.0 + pan.0, r[1] + r[3] / 2.0 - h / 2.0 + pan.1);
            // The fitted picture stretched under it, then a sharp one of what shows, once still.
            let (pw, ph) = px_size(r[2], r[3], app.canvas.u);
            let (b0, base) = app.st.designs.key(&plan, &What::Board(bd.id.clone()), pw, ph);
            let old = app.st.designs.shown.get(&b0).cloned();
            let whole = [0.0, 0.0, 1.0, 1.0];
            if !app.canvas.image_part(&base, x, y, w, h, 0.0, Rgba::WHITE, whole) && !old.is_some_and(|k| app.canvas.image_part(&k, x, y, w, h, 0.0, Rgba::WHITE, whole)) {
                app.canvas.rect(x, y, w, h, 0.0, t::S1);
            }
            let (vx0, vy0) = (x.max(area[0]), y.max(area[1]));
            let (vx1, vy1) = ((x + w).min(area[0] + area[2]), (y + h).min(area[1] + area[3]));
            if still && vx1 > vx0 && vy1 > vy0 {
                let k = bd.rect.w as f32 / w;
                let region = Rect::new(bd.rect.x + f64::from((vx0 - x) * k), bd.rect.y + f64::from((vy0 - y) * k), f64::from((vx1 - vx0) * k), f64::from((vy1 - vy0) * k));
                picture(app, &plan, What::Region(Some(bd.id.clone()), region), vx0, vy0, vx1 - vx0, vy1 - vy0, 0.0, true);
            }
            continue;
        }
        if picture(app, &plan, What::Board(bd.id.clone()), r[0] + off, r[1], r[2], r[3], 4.0, true).is_none() {
            app.canvas.rect(r[0] + off, r[1], r[2], r[3], 4.0, t::S1);
        }
        app.canvas.outline(r[0] + off, r[1], r[2], r[3], 4.0, 1.0, t::LINE);
    }
    app.canvas.pop_clip();

    // Where you are among them.
    let dots_y = area[1] + area[3] + 15.0;
    if (2..=12).contains(&n) {
        let widths: Vec<f32> = (0..n).map(|i| if i == at { 16.0 } else { 6.0 }).collect();
        let total: f32 = widths.iter().sum::<f32>() + 6.0 * (n as f32 - 1.0);
        let mut dx = (WIDTH - total) / 2.0;
        for (i, dw) in widths.iter().enumerate() {
            app.canvas.rect(dx, dots_y, *dw, 6.0, 3.0, if i == at { t::INK } else { t::S3 });
            dx += dw + 6.0;
        }
    }

    let sub = format!("{} · {} of {n} · {} × {}", plan.page_name, at + 1, b.rect.w.round(), b.rect.h.round());
    let (close, _) = bar(app, "close", &plan.boards[at].name, &sub, None);
    if close && let Some(o) = app.st.designs.open.as_mut() {
        o.view = None;
    }
    moving || app.st.designs.busy() && !crate::wake::has()
}

// Sample data for screenshots

/// A small design page with three artboards: what the shots and the tests draw.
pub fn sample_page() -> Value {
    let ink = "#F5F4EF";
    json!({
        "design": {"id": 44, "rev": 12, "title": "Relay Code · phone app rewrite"},
        "colors": {"accent": "#D97757", "card": "#2A2927"},
        "nodes": [
            {"id": "pg", "kind": "page", "name": "Chat", "fill": "#141413"},
            {"id": "home", "kind": "frame", "parent": "pg", "name": "Home", "x": 0, "y": 0, "w": 390, "h": 844, "fill": "#1F1E1D", "radius": 0,
             "layout": "column", "padding": [56, 16, 24, 16], "gap": 12},
            {"id": "h-title", "kind": "text", "parent": "home", "text": "Designs", "font_size": 24, "font_weight": 600, "color": ink},
            {"id": "h-body", "kind": "text", "parent": "home", "text": "Every design an agent draws for this repo, newest first, with its artboards.", "w_sizing": "fill", "font_size": 15, "color": "#B5B0A6"},
            {"id": "h-card", "kind": "rect", "parent": "home", "w": 358, "h": 120, "fill": "$card", "radius": 12, "text": "Relay Code · phone app", "color": ink, "font_size": 16},
            {"id": "h-pic", "kind": "image", "parent": "home", "w": 358, "h": 150, "src": SAMPLE_SRC, "radius": 12},
            {"id": "h-row", "kind": "frame", "parent": "home", "w": 358, "h": 48, "fill": "#00000000", "layout": "row", "gap": 12, "items": "center"},
            {"id": "h-dot", "kind": "ellipse", "parent": "h-row", "w": 36, "h": 36, "fill": "#8CC08F"},
            {"id": "h-icon", "kind": "icon", "parent": "h-row", "icon": "search", "w": 22, "h": 22, "stroke": ink},
            {"id": "h-name", "kind": "text", "parent": "h-row", "text": "bold-puffin", "font": "Geist Mono", "font_size": 14, "color": ink},
            {"id": "h-key", "kind": "rect", "parent": "home", "w": 358, "h": 46, "fill": "$accent", "radius": 10, "text": "Open", "color": "#1F1E1C", "font_weight": 600},
            {"id": "chat", "kind": "frame", "parent": "pg", "name": "Chat", "x": 470, "y": 0, "w": 390, "h": 844, "fill": "#1F1E1D",
             "gradient": {"kind": "linear", "angle": 180, "stops": [{"at": 0, "color": "#2A2927"}, {"at": 1, "color": "#1F1E1D"}]}},
            {"id": "c-bubble", "kind": "rect", "parent": "chat", "x": 120, "y": 80, "w": 250, "h": 64, "fill": "#353431", "radii": [16, 16, 4, 16],
             "text": "Bubbles duplicate when I send. Fix it?", "color": ink, "font_size": 14, "align": "left"},
            {"id": "c-words", "kind": "text", "parent": "chat", "x": 20, "y": 170, "w": 350, "text": "Found it: the phone matched its own copy to the log by the words.", "font": "Literata", "font_size": 16, "color": ink, "line_height": 1.5},
            {"id": "c-star", "kind": "polygon", "parent": "chat", "x": 30, "y": 300, "w": 80, "h": 80, "sides": 5, "star": 0.45, "fill": "#E5B567"},
            {"id": "c-ring", "kind": "ellipse", "parent": "chat", "x": 150, "y": 300, "w": 80, "h": 80, "fill": "#D97757", "arc": {"start": 0, "sweep": 250, "inner": 0.7, "cap": "round"}},
            {"id": "c-line", "kind": "line", "parent": "chat", "x": 260, "y": 300, "w": 100, "h": 80, "points": [[0, 80], [100, 0]], "stroke": ink, "stroke_width": 2, "arrow_end": true},
            {"id": "c-note", "kind": "sticky", "parent": "chat", "x": 30, "y": 420, "w": 200, "h": 120, "text": "Ask sends this frame with its picture."},
            {"id": "c-box", "kind": "frame", "parent": "chat", "x": 16, "y": 760, "w": 358, "h": 56, "fill": "#2A2927", "radius": 14, "stroke": "#FFFFFF14", "stroke_width": 1,
             "shadow": {"x": 0, "y": 4, "blur": 12, "color": "#00000066"}},
            {"id": "c-send", "kind": "ellipse", "parent": "c-box", "x": 310, "y": 10, "w": 36, "h": 36, "fill": "#D97757"},
            {"id": "wide", "kind": "frame", "parent": "pg", "name": "Desktop", "x": 940, "y": 0, "w": 1200, "h": 700, "radius": 16,
             "gradient": {"kind": "radial", "stops": [{"at": 0, "color": "#353431"}, {"at": 1, "color": "#181716"}]}},
            {"id": "w-title", "kind": "text", "parent": "wide", "x": 40, "y": 40, "text": "Relay on the desktop", "font_size": 40, "font_weight": 600, "color": ink},
            {"id": "w-rule", "kind": "rect", "parent": "wide", "x": 40, "y": 110, "w": 1120, "h": 1, "fill": "#FFFFFF24"},
            {"id": "w-dash", "kind": "rect", "parent": "wide", "x": 40, "y": 140, "w": 400, "h": 220, "fill": "#2A2927", "radius": 12, "stroke": "#9E998F", "stroke_width": 2, "dash": [6, 4]},
            {"id": "w-pic", "kind": "image", "parent": "wide", "x": 480, "y": 140, "w": 400, "h": 220, "src": SAMPLE_SRC, "fit": "contain", "flip_x": true, "radius": 12,
             "stroke": "#FFFFFF24", "stroke_width": 1}
        ]
    })
}

/// Sample designs for the shots (`--shot`): the list (E1), design 44 open (E3); see
/// [`sample_view`] for E4. Everything is painted in the frame (`inline`).
pub fn sample(app: &mut App) {
    if pictures::status(SAMPLE_SRC) != pictures::Status::Ready {
        pictures::put(SAMPLE_SRC, pictures::decode(SAMPLE_PICTURE, (0, 0)));
    }
    let plan = Arc::new(dp::plan(44, sample_page()).expect("the sample page plans"));
    let now = rc_link::link::now_ms();
    let at = |mins: u64| {
        let s = (now / 1000).saturating_sub(mins * 60);
        let (d, r) = (s / 86_400, s % 86_400);
        // Days to a civil date, as sample.rs does.
        let z = d as i64 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + i64::from(m <= 2);
        format!("{y:04}-{m:02}-{day:02}T{:02}:{:02}:{:02}Z", r / 3600, r / 60 % 60, r % 60)
    };
    let list = items(&json!({"designs": [
        {"id": 44, "title": "Relay Code · phone app rewrite in Rust", "rev": 12, "pages": 4, "session": "bold-puffin", "updated_at": at(0)},
        {"id": 43, "title": "Relay, one page · chat-first", "rev": 30, "pages": 1, "session": "sunny-egret", "updated_at": at(60 * 50)},
        {"id": 42, "title": "Leaf · reading app", "rev": 80, "pages": 19, "session": "swift-ocelot", "updated_at": at(60 * 26)},
        {"id": 41, "title": "Old idea", "rev": 3, "pages": 1, "updated_at": at(5), "deleted_at": at(1)},
    ]}));
    let mut st = State { repo: app.repo, loaded: true, inline: true, ..State::default() };
    for i in &list {
        st.thumbs.insert(i.id, Thumb { rev: i.rev, stage: Stage::Ready, plan: Some(plan.clone()), key: None });
    }
    st.list = list;
    st.open = Some(Open {
        repo: app.repo,
        id: 44,
        title: "Relay Code · phone app rewrite in Rust".into(),
        session: Some("bold-puffin".into()),
        updated_at: at(0),
        rev: 12,
        pages: vec![("p1".into(), "Decisions".into()), ("p2".into(), "Getting around".into()), ("pg".into(), "Chat".into()), ("p4".into(), "Designs".into())],
        page: Some("pg".into()),
        plan: Some(plan.clone()),
        ..Open::default()
    });
    st.plans.push(plan);
    app.st.designs = st;
}

/// E4 for the shots: the second artboard of the open sample design, full screen.
pub fn sample_view(app: &mut App) {
    if let Some(o) = app.st.designs.open.as_mut() {
        o.view = Some(View { at: 1, zoom: 1.0, ..View::default() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frames drawn at rest on `route`, after `warm` to settle: µs a frame (release, this machine).
    fn frame_cost(a: &mut App, warm: usize, n: usize) -> f64 {
        let mut clock = 2_000_000_000u64;
        for _ in 0..warm {
            clock += 16_666_666;
            a.frame(clock);
        }
        let t = std::time::Instant::now();
        for _ in 0..n {
            clock += 16_666_666;
            a.frame(clock);
        }
        t.elapsed().as_secs_f64() * 1e6 / n as f64
    }

    fn bench_app(name: &str) -> App {
        struct NoHost(std::path::PathBuf);
        impl crate::app::Host for NoHost {
            fn data_dir(&self) -> std::path::PathBuf {
                self.0.clone()
            }
            fn log(&self, _text: &str) {}
        }
        let dir = std::env::temp_dir().join(format!("relay-code-bench-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(NoHost(dir)), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        a
    }

    /// The Designs list with 40 designs, at rest:
    /// `cargo test --release designs_frame_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn designs_frame_cost() {
        let mut a = bench_app("designs");
        let plan = a.st.designs.thumbs.values().find_map(|t| t.plan.clone()).expect("a sample plan");
        let list: Vec<Item> = (0..40).map(|i| Item { id: 1000 + i, title: format!("Design {i} · a page of the app"), rev: 1, pages: 3, session: Some("bold-puffin".into()), updated_at: String::new() }).collect();
        for i in &list {
            a.st.designs.thumbs.insert(i.id, Thumb { rev: 1, stage: Stage::Ready, plan: Some(plan.clone()), key: None });
        }
        a.st.designs.list = list;
        a.push(crate::app::Route::Designs);
        println!("designs frame, 40 designs: {:.0} µs", frame_cost(&mut a, 20, 200));
    }

    #[test]
    fn designs_list_newest_first_without_the_deleted() {
        let v = json!({"designs": [
            {"id": 1, "title": "Old", "rev": 2, "pages": 1, "updated_at": "2026-11-20T10:00:00Z"},
            {"id": 2, "title": "Gone", "rev": 2, "pages": 1, "updated_at": "2026-11-25T10:00:00Z", "deleted_at": "2026-11-25T11:00:00Z"},
            {"id": 3, "title": "New", "rev": 9, "pages": 4, "session": "bold-puffin", "updated_at": "2026-11-24T10:00:00Z"},
        ]});
        let list = items(&v);
        assert_eq!(list.iter().map(|i| i.id).collect::<Vec<_>>(), [3, 1]);
        let now = crate::data::parse_ms("2026-11-24T10:05:00Z").unwrap();
        assert_eq!(subtitle(&list[0], now), "4 pages · by bold-puffin · 5m ago");
        assert_eq!(subtitle(&Item { pages: 1, session: None, ..list[0].clone() }, crate::data::parse_ms("2026-11-24T10:00:10Z").unwrap()), "1 page · now");
    }

    #[test]
    fn pages_read_in_order_with_names() {
        let v = json!({"nodes": [{"id": "a", "kind": "page", "name": "1 · Decisions"}, {"id": "b", "kind": "page", "name": " "}, {"id": "f", "kind": "frame"}]});
        assert_eq!(pages(&v), [("a".to_string(), "1 · Decisions".to_string()), ("b".to_string(), "Page 2".to_string())]);
    }

    #[test]
    fn pictures_stay_within_what_the_atlas_takes() {
        assert_eq!(px_size(100.0, 50.0, 2.0), (200, 100));
        let (w, h) = px_size(390.0, 844.0, 3.0);
        assert!(h as f32 <= MAX_SIDE && w > 0);
        assert!((w as f32 / h as f32 - 390.0 / 844.0).abs() < 0.01);
    }

    #[test]
    fn an_artboard_rests_fitted_and_pans_only_when_zoomed() {
        let area = [0.0, 100.0, 390.0, 700.0];
        let r = rest_box(390.0, 844.0, area);
        assert!(r[3] <= 676.0 + 1e-3 && r[2] <= 366.0 + 1e-3);
        assert!((r[0] + r[2] / 2.0 - 195.0).abs() < 1e-3);
        assert_eq!(pan_limit(r[2], r[3], 1.0, area), (0.0, 0.0));
        let (lx, ly) = pan_limit(r[2], r[3], 2.5, area);
        assert!(lx > 0.0 && ly > 0.0);
    }

    #[test]
    fn back_closes_the_artboard_then_the_whole_page() {
        let mut st = State { open: Some(Open { whole: true, view: Some(View::default()), ..Open::default() }), ..State::default() };
        assert!(st.close_over());
        assert!(st.open.as_ref().unwrap().view.is_none());
        assert!(st.close_over());
        assert!(!st.close_over());
    }

    #[test]
    fn the_worker_plans_and_paints_off_the_thread() {
        let mut st = State::default();
        st.submit(Job::Plan { tag: Tag::Thumb(44), design: 44, value: sample_page() });
        let mut got = None;
        for _ in 0..500 {
            if let Some(d) = st.worker.as_ref().and_then(|w| w.rx.try_recv().ok()) {
                got = Some(d);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        st.handle(got.expect("the worker answers"));
        let plan = st.thumbs[&44].plan.clone().expect("planned");
        assert_eq!(plan.rev, 12);
        st.asked.insert("k".into());
        st.submit(Job::Paint { key: "k".into(), plan, what: What::Strip, w: 120, h: 50 });
        for _ in 0..500 {
            if let Some(d) = st.worker.as_ref().and_then(|w| w.rx.try_recv().ok()) {
                st.handle(d);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(st.asked.is_empty());
        assert_eq!(st.pics.len(), 1);
        assert_eq!(st.pics[0].1.data.len(), 120 * 50 * 4);
    }

    #[test]
    fn after_a_pan_only_the_newest_part_of_an_artboard_paints() {
        let plan = Arc::new(dp::plan(44, sample_page()).expect("plans"));
        let region = |k: &str, root: &str| Job::Paint { key: k.into(), plan: plan.clone(), what: What::Region(Some(root.into()), Rect::new(0.0, 0.0, 10.0, 10.0)), w: 10, h: 10 };
        let board = Job::Paint { key: "board".into(), plan: plan.clone(), what: What::Board("a".into()), w: 10, h: 10 };
        let mut q: VecDeque<Job> = [region("a1", "a"), board, region("a2", "a"), region("b1", "b"), region("a3", "a")].into();
        let key = |j: Option<Job>| match j {
            Some(Job::Paint { key, .. }) => key,
            _ => String::new(),
        };
        let (j, dropped) = next_job(&mut q);
        assert_eq!((key(j), dropped), ("board".to_string(), vec!["a1".to_string()]));
        let (j, dropped) = next_job(&mut q);
        assert_eq!((key(j), dropped), ("b1".to_string(), vec!["a2".to_string()]));
        let (j, dropped) = next_job(&mut q);
        assert_eq!((key(j), dropped), ("a3".to_string(), vec![]));
        // A skipped part may be asked for again.
        let mut st = State::default();
        st.asked.insert("a1".into());
        st.handle(Done::Dropped("a1".into()));
        assert!(st.asked.is_empty() && st.failed.is_empty());
    }

    #[test]
    fn a_page_left_while_it_was_laid_out_does_not_keep_the_screen_busy() {
        let mut st = State { open: Some(Open { id: 7, page: Some("b".into()), planning: Some("a".into()), ..Open::default() }), ..State::default() };
        assert!(st.busy());
        st.handle(Done::Plan(Tag::Open(7, "a".into()), Err("no".into())));
        let o = st.open.as_ref().unwrap();
        assert!(o.planning.is_none() && o.error.is_none(), "page a's answer clears its own wait, and says nothing over page b");
        assert!(!st.busy());
    }

    #[test]
    fn a_thumbnail_scrolled_away_lets_its_layout_go() {
        let mut st = State::default();
        let plan = Arc::new(dp::plan(44, sample_page()).expect("plans"));
        st.thumbs.insert(44, Thumb { rev: 12, stage: Stage::Ready, plan: Some(plan.clone()), key: None });
        st.keep_plan(plan.clone());
        st.let_go(44);
        assert!(st.thumbs[&44].plan.is_none() && st.plans.is_empty());
        assert_eq!(Arc::strong_count(&plan), 1, "nothing holds it but this test");
        // With no strip painted it is read again when it shows.
        assert!(matches!(st.thumbs[&44].stage, Stage::Idle));
    }

    #[test]
    fn picking_another_repo_on_a_design_leaves_for_the_list() {
        struct NoHost(std::path::PathBuf);
        impl crate::app::Host for NoHost {
            fn data_dir(&self) -> std::path::PathBuf {
                self.0.clone()
            }
            fn log(&self, _text: &str) {}
        }
        let dir = std::env::temp_dir().join(format!("relay-code-design-repo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(Arc::new(NoHost(dir)), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        a.sample();
        a.push(crate::app::Route::Designs);
        a.push(crate::app::Route::Design(44));
        let mut clock = 2_000_000_000u64;
        for _ in 0..2 {
            clock += 16_666_666;
            a.frame(clock);
        }
        assert_eq!(*a.route(), crate::app::Route::Design(44));
        a.repo = Some(2);
        for _ in 0..2 {
            clock += 16_666_666;
            a.frame(clock);
        }
        assert_eq!(*a.route(), crate::app::Route::Designs);
        assert!(a.st.designs.open.is_none());
    }

    /// The screens drawn by the GPU from sample data, as `--shot` draws them:
    /// `DESIGN_SHOTS=dir cargo test -p relay-code designs::tests::shots -- --ignored`.
    #[test]
    #[ignore]
    #[cfg(not(target_os = "android"))]
    fn shots() {
        struct Host(std::path::PathBuf);
        impl crate::app::Host for Host {
            fn data_dir(&self) -> std::path::PathBuf {
                self.0.clone()
            }
            fn log(&self, _: &str) {}
        }
        let Ok(dir) = std::env::var("DESIGN_SHOTS") else { return };
        let (w, h) = (1080u32, 2340u32);
        for screen in ["designs", "design", "whole", "artboard", "zoomed"] {
            let mut gfx = crate::gfx::Gfx::headless().unwrap();
            let data = std::env::temp_dir().join(format!("relay-code-design-shot-{}", std::process::id()));
            let mut clock = 1_000_000_000u64;
            let mut app = App::new(Arc::new(Host(data.clone())), clock);
            app.canvas.set_images_size(gfx.images_size());
            app.resize(w as f32, h as f32, w as f32 * 0.085, w as f32 * 0.04);
            app.sample();
            sample(&mut app);
            app.reset_to(crate::app::Route::Home);
            app.push(crate::app::Route::Designs);
            if screen != "designs" {
                app.push(crate::app::Route::Design(44));
            }
            if screen == "whole" {
                app.st.designs.open.as_mut().unwrap().whole = true;
            }
            if screen == "artboard" || screen == "zoomed" {
                sample_view(&mut app);
            }
            if screen == "zoomed" {
                let v = app.st.designs.open.as_mut().unwrap().view.as_mut().unwrap();
                v.zoom = 2.5;
                v.pan = (150.0, 200.0);
            }
            for _ in 0..21 {
                clock += 16_666_666;
                app.frame(clock);
                gfx.upload_glyphs(&mut app.canvas.uploads);
                gfx.upload_images(&mut app.canvas.image_uploads);
                gfx.flush();
            }
            let pixels = gfx.render_to_pixels(w, h, &app.canvas.globals, app.clear_colour(), &app.canvas.insts, &app.canvas.batches);
            let file = std::fs::File::create(format!("{dir}/{screen}.png")).unwrap();
            let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&pixels).unwrap();
            let _ = std::fs::remove_dir_all(&data);
        }
    }
}
