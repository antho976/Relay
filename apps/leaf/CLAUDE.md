# Leaf: `apps/leaf`

Leaf is Antho's reading app: Rust drawn with wgpu (like Tally RS), for Android and a desktop
window. **Books** (EPUB, PDF, text) are imported from the phone and kept in a local SQLite shelf;
**comics** are fetched by the owner's server. The approved mockups are Relay design 42 ("Leaf ·
reading app"). What is built and how: `README.md`.

## Two halves, one workspace

`native/` is its own Cargo workspace with its own `Cargo.lock`, not a member of the root one:
run cargo from there, and plain `cargo test` at the root never builds it.

| Crate | What |
|---|---|
| `leaf-proto` | The wire types the phone and the server share, and the rules both must read alike: title matching, gaps, which copy of a chapter to read. |
| `leaf-server` | The comics autopilot beside a Suwayomi it drives (`server/compose.yml`). Headless: `autopilot` (the pass), `budget` (per-source requests), `thumbs`, `recs` (AniList), `webtoon` (paid episodes), `api`. |
| `leaf-app` | The phone app (wgpu; renderer copied from Tally RS): tabs Home, Search, My space (`home`, `search`, `space`); books (`shelf`, `epub`, `typeset`, `book`, `notes`, `stats`), comics (`series`, `reader`), picks (`recs`), app updates (`update`), the offline prompt (`online`) and drag to select (`sweep`). |

```fish
cd apps/leaf/native
cargo test
cargo clippy --all-targets -- -D warnings
```

## The promise that is easy to break

Following a series must need nothing more from the owner: the autopilot picks the source,
keeps chapters downloaded ahead, and moves a series to another source when its own runs out or
stops answering, carrying the place across. It moves only for a clear gain (`decide` in
`autopilot.rs`, with tests): never trade places between sources that keep pace, and keep the
translation being read until it runs out. A change there keeps those tests and adds one.

Suwayomi is never exposed: leaf-server is the only door, and it needs `LEAF_TOKEN` to listen
anywhere but loopback.

## The server asks each source politely (2026-11-23)

The owner's IP must not get banned. Everything in leaf-server that reaches a site through
Suwayomi (a search, `refresh`, `pages`, page pictures not downloaded, `enqueue`) or AniList goes
through `Ctx::ask` with its source's name: `Lane::Back` for the autopilot (waits its turn, skipped
while the source is paused), `Lane::Fore` only for what the phone is waiting on. A new call that
reaches a site goes through it too, a background loop never asks every series at once (`plan`
spreads the pass), and downloads go through the line (`queue_downloads`, drained per source), not
straight to Suwayomi. An error starting with `PAUSED` is not the source's failure: it never counts
toward moving a series. Chapter pictures come only from Suwayomi's downloaded files. Tests of
these rules pass time in; none sleep.

Every source has ceilings below what its provider publishes, and modest ones where it publishes
nothing (`budget::limits_for`, tested against the published numbers): per second and per minute for
both lanes together, with pictures, page lists and the rest counted apart, and a day's share for the
background. The phone's request waits a moment over a ceiling and is told "busy" past 5 s; while a
source is paused it gets a quarter of its ceilings. A new provider gets its own row there.

Search asks only the broad sources (`LEAF_SEARCH_SOURCES`) unless the owner asks for all, keeps
each source's answer 10 minutes, and the phone searches only after typing pauses. Promo pages are
recognised only from page bytes already passing through (`promo.rs`): never fetch a page to hash it,
except the one page the owner hides by hand.

`Series` and `Chapter` gain fields only through JSON the server adds (`paid`, `thumb`): leaf-server
builds both structs field by field (`autopilot.rs`), and leaf-app in its tests, so a new field on
them breaks both builds. The phone reads `paid` and `thumb` beside the structs, from the same JSON
(`net::Extras`, 2026-11-23); moving them into the structs is a change to both crates at once.

## Books: what is easy to break

- A place and a highlight are (section, block, byte) into the parsed EPUB, and stored that way in
  `shelf.db`: `epub.rs` must keep giving the same blocks for the same file. A change to how XHTML
  becomes blocks moves every saved place and highlight; don't make one without a migration plan.
- Nothing slow on the UI thread: a book is parsed on a worker, set in lines only near the screen,
  and every shelf write goes through the shelf's thread. Shots (`--shot`) run capped:
  `systemd-run --user --scope -q -p MemoryMax=3G -p MemorySwapMax=0 target/debug/leaf --shot ...`.
- Reading time counts only while the page moves (`book::ReadClock`); don't count frames.
- Shots of books use public-domain EPUBs downloaded into `.dev/` (never committed).

## The phone's UI: settled (design 42, 2026-11-22)

- Tabs are Home, Search and My space only; Notes and Stats are not tabs (My space's ⋯ reaches
  them). My space's own tabs are Updates (first), Recent, Following, Downloads (2026-11-23). No
  source names anywhere in the UI, except Settings' Sources list, which the owner asked for
  (2026-11-23). Adding a series is not following it: the bell is.
- A download is a copy on this phone for reading offline (`offline.rs`, `offline/<chapter>/`, apart
  from the page cache, which may be trimmed): nothing downloads without being asked. What the
  server keeps (its one next chapter, chapters it fetched for the phone's line) is plumbing and is
  never shown as a download; Downloads and the rows' states come from the phone's folder alone,
  and deleting one never asks the server (2026-11-23). Its line goes one chapter at a time and
  puts a chapter the server lacks in the server's own line first, so pages come from the server's
  disk; select bars (`saved::select_bar`) are fixed chrome, tested in `flows.rs`.
- While a menu is open, the page under it takes no tap (`kit::hold`; `flows.rs` tests it). Nor
  does a tap on fixed chrome drawn after the page (the tab bar, a fixed top): `Ui::begin_frame`
  holds it and `kit::chrome` / `kit::fixed_top` give it back; new fixed chrome registers itself so.
- The spotlight is never dragged: it rests 18 s on each, slides gently, and its dots choose one.
  It is a rounded card on Home (design 42 page 15, 2026-11-23), and only covers that fill it sharp
  take a turn. No cover is drawn past 1.3× its own pixels (`images::MAX_UPSCALE`).
- My space rows and chapter rows use smaller pictures than the artboards draw (the owner,
  2026-11-23): square covers 88 (`space::ROW_H`), chapter pictures 72 × 54 (`series::PIC`).
- Chapter pictures come only from the server (`/v1/chapters/{id}/thumb`): the phone never fetches
  pages to make one.
- The reader (2026-11-24): a drag that starts on the phone's gesture strips (the top and bottom
  28 units, or the bars if taller) never scrolls the strip. The phone's bars (time, battery) show
  while the overlay is up and hide with it (`App::immersive`, books too); headings keep the
  bars-hidden inset so nothing moves when they come. The saved place only moves down the strip
  (`Reader::further`): scrolling back up leaves it, and the server's page, where reading got to.
  With the overlay up it is marked where it is on the strip, a line and a hanging "BOOKMARK" tag;
  the header names it only when it is in another chapter (the owner, 2026-11-24).
- A tap in the reader (comics and books) only shows or hides the overlay: no tap zones and no
  volume-key paging, and no setting for either (the owner, 2026-11-23).
- A search result or a pick (Top picks, Because you read, Collections, Readers also like) opens its
  page on a tap: a preview (`/v1/preview`, a negative id, `App::open_preview`) with Follow and the
  download key beside it, nothing added. Reading a chapter, Follow or a download adds it
  (`App::adopt`: `/v1/follow` with its entries, then the chapter or the downloads by number). A
  long press on a pick opens its sheet (why, Follow, Read chapter 1, Not interested, Fewer like).
  A series shows in My space and on Home only once a chapter is read, it is followed or something
  is downloaded (`logic::in_space`); opening a page adds nothing (2026-11-23).
- Every sheet with a grab bar is pulled down by it (`kit::sheet`, `Ui::drag_sheet`): it follows
  the finger, closes when let go past 30% of its height or flicked, else springs back; a sheet with
  a list inside is pulled by its head (`kit::sheet_grab`). A drag on a sheet never moves the page or
  a rail under it. A sheet holds every tap from the page under it; the series page's bottom key is
  fixed chrome (`kit::chrome`). `flows.rs` tests each.
- Home has no "Finished, to binge" and no Collections (the server still makes both; the owner
  never used Collections, 2026-11-24), and no Because rail of fewer than two picks; each Because
  rail has a banner, a still collage of its covers. Nothing there moves (2026-11-23). Top picks
  show six of the server's 24, the next six every four hours (`logic::rotating`, 2026-11-24).
- Out of reach (no network, the server off, or a proxy saying it is down: `Fail::out_of_reach`,
  read from every live answer by `Done::reach`), a sheet says "You're offline" once a spell
  (`online.rs`): the cloud struck through, what still opens, Open downloads (My space › Downloads)
  and Not now, or OK alone when nothing is downloaded. Never over a strip or a book being read (it
  waits until the reader closes); not again until the server answered and was lost again; "Back
  online" is a toast. While out of reach the server is tried every 20 s and on coming back to the
  front; the platform's network state is not read (2026-11-23).
- Recent (2026-11-25): what was read here, last read first; then series partway through but never
  opened here (progress that came with them), fewest chapters left first, saying "2 chapters left"
  and Continue rather than "Not started"; then caught up; then not started (`logic::recent_key`).
- Next up (2026-11-25): under the end of the newest chapter, the series in My space with the
  fewest chapters left (not On hold, Dropped or Done), opened where you are (`logic::next_up`).
- Imported progress (2026-11-25): a series page with unread chapters below the furthest read shows
  "N chapters before X are unread · Mark read" (`logic::holes`); ⋯ → "I've read up to…" marks every
  chapter to N read and every one after unread (`ReadBefore::unread_after`).
- Books are copied to the server once a day (`booksync.rs`, `leaf-server/src/phone.rs`): the
  books, covers and a `VACUUM INTO` copy of `shelf.db`, under `<LEAF_DATA>/phone/`. A phone that
  never made a copy never sends its shelf over one already there: Settings offers Restore (or "Use
  this phone's"). Restore replaces the shelf's rows in one transaction (`shelf::Op::Restore`).
  Files over 95 MB stay on the phone (Cloudflare takes 100 MB at most).
- Followed twice (2026-11-25): the server pairs followed series whose titles or aliases name one
  work (`GET /v1/duplicates`, `duplicate_pairs`); My space's Recent and Following show "N series are
  in My space twice", and Merge keeps the one read further (`logic::merge_keep`, `POST
  /v1/series/{id}/merge`: the other's copies become links, its bell, shelf and progress carried).
  "Not the same" is kept for good (`notdup:` in kv).
- "Download Next up on Wi-Fi" (Settings › Reading, off by default, 2026-11-25): the next 2
  chapters of the 3 series closest to their end go in the download line, quietly, every 3 hours,
  only on an unmetered network (`Host::unmetered`, `LeafActivity.unmetered`; the one place the
  platform's network state is read). It is the owner's ask for those downloads.
- A series' description (2026-11-25, design 42 page 18: B on the page, A in the sheet) is taken
  apart by `logic::about`: the source's rating and bookmarks become readings (★ 9.29 · 41.1K
  bookmarks · Ongoing under the title), a bracketed tagline its own line, a quotation standing as
  its own sentence a pull quote, "Alternative Titles" a list. The page shows the tagline, two lines
  and More (opening in place); ⓘ shows it all with the readings in a row.
- A tap on the series page's cover opens it full screen (`zoom.rs`): at its own size (never past
  the screen), two fingers zoom to 5x, a double tap zooms in and back, a pull down closes. The
  picture is the cover's own file (`Images::full`). `Ui` follows a second finger for this
  (`Ui::pinch`); nothing else uses it yet.
- A pick opens with what it knows (2026-11-25): `Preview::stand_in` (title, status) and AniList's
  description (`Rec::description`, from the picks' own AniList query: no extra request) show at
  once, the pick's cover stands in for the entry's, and the chapters fill in as the server answers.
  The server asks for Top picks' pages (and the first six of each Because rail) as it makes the
  day's picks, in the background lane, and keeps them a day (`warm_previews`, `WARM_KEEP_MS`).
- Your stars (2026-11-25): a series' readings line ends with "You gave 4" or "Rate"; a tap opens
  the sheet (`POST /v1/series/{id}/rating`, kept as `stars:{id}` on the server). Picks weigh a
  series by them (`recs::weight`: 1–2 stars point nowhere and lead no rail, 4 counts 1.4, 5
  counts 2), and "Because you finished X" is the series finished last (`recs::recent`), from the
  same AniList answer and within the same daily search allowance: no request more.
- When chapters come (`rhythm.rs`): the server sends each series' newest upload dates beside it
  (`uploads`, one a day, 12 at most) and its genres (`genres`, Suwayomi's own); the phone says
  "New chapter usually Wednesday" under the readings, lists "Due today" atop Updates, and a series
  six weeks without a chapter says "On a break" in Following (`leaf_proto::BREAK_MS`). Following
  has genre chips under the shelves (`logic::top_genres`).
- The reader (2026-11-25): two fingers zoom the strip (to `reader::ZOOM_MAX`, one finger then
  moves it across; no double tap: a tap still only shows the overlay); Warmth, an amber veil, sits
  under Brightness (books too). A long press on a page offers Save picture (what of the page is
  on screen, kept in `panels/` and put in Pictures/Leaf, `panels.rs`; the series page shows
  "Saved pictures", the viewer's trash key lets one go from Leaf), "This page is broken: read it
  elsewhere" (`POST /v1/chapters/{id}/elsewhere`: the chapter is read from another copy from then
  on, `elsewhere:{chapter}`), and Hide.
- Search lists what is in My space first ("In My space"); the very title of one of yours asks no
  source until "Search your sources too". Searches that led to a page are kept (8) and shown under
  the field. Words shared to Leaf (a link or a title, Android's share sheet) open Search with the
  title (`logic::shared_title`).
- Settings: "Delete downloads once read" (when leaving the reader), "Open where I left off"
  (`reading_now` in config), and Server: the last pass, the download line, the room chapters take
  and its size (`POST /v1/settings/disk`; `disk.rs`: at 90% of it the chapters read longest ago
  go down to 80%, never an unread one nor the last two read of a series), the nightly backup and
  the copy on the owner's PC (`tools/pull-backup.sh`, installed by `tools/install-offsite.sh` as a
  user timer; the server learns of it from `offsite_at`).
- Settings is one page (the owner, 2026-11-26), in this order: Reading, Downloads (this phone's),
  Notifications, Server (its address in a row with Change, which folds out the address and token;
  the pass, the line and the room), Sources, AniList (setup steps folded behind Set up), Backups
  (the nightly one, the PC copy, export, your books), the build last. The full Connect form is the
  first run's alone (`connect.rs`).
- Notices: "finished" (a belled series the source now calls completed) and "blocked" (no source
  answers) post on their own; new chapters of one series carry Read now and Download keys
  (`LeafChaptersJob`, `App::launched`). No source's name in either.
- Wrapped (design 45, 2026-11-25; `wrapped.rs`, numbers in `wrapped_stats.rs`): the month's, and
  the year's, as cards tapped through. The only place reading numbers show. From
  `/v1/history` (Suwayomi's own read times, chapters marked in bulk left out) and the books'
  clocked time; a comic's time is told from the gaps between chapters ("about"). Home shows a card
  in a month's first week (the year's in January's first two), first until opened; My space's ⋯
  has last month's; a notice on the 1st at 9 says it is ready. Share sends the readings as text.
- Every select mode selects in bulk by sweeping (`sweep.rs`, the owner, 2026-11-23): a long press on
  a row and a drag without lifting selects the range from that row to the finger, dragging back lets
  go, a long press on a selected row de-selects, the list scrolls by itself in a 72-unit band at its
  edges, and a haptic tick marks each row caught. A drag that did not start with a long press
  scrolls (`Ui::claim` holds lists still only for a sweep). A list that selects records its rows
  and calls `App::sweep_rows` after `page_end`; `flows.rs` tests each list.
- Long presses are timed on the frames' clock (`Ui::begin_frame` stamps the touch), so tests pass
  time in rather than sleep.
- The app asks for a newer build when it opens and whenever it comes back to the front, at most
  every 15 minutes (`update::due`); Later still hides that build for a day (2026-11-23).
- Webtoon (2026-11-23): a series' paid row ("Chapters 9–23 are on Webtoon (paid)") is cut at send
  time to what the series has (`webtoon::still_paid`), so a chapter another source brings drops out
  of it at once; each Webtoon list page is read at most once a day (`wt:{url}`). A search result
  only Webtoon has, whose newer chapters are sold, says "Webtoon only, chapters 8–23 paid" under its
  title (`Candidate::paid`, at most four pages read per search). A series only Webtoon has gets a
  row under the paid one, "Tell me when another source has it" (Watch): the daily comparison posts
  one "elsewhere" notice when one does, never folded into the daily summary, and the watch ends.
- The reader decodes pages five screens ahead (`reader::DECODE_AHEAD`): failures shown only
  within two screens, the rest tried again as they come near. Pages only kept on disk ahead never
  take the last free page worker (`images::PLAIN_AT_ONCE`), so the page you scroll to never waits
  on a slow network (2026-11-23).
- Series ids are never reused by the server (`work_next_id` in `kv`); the phone still lets go of
  what it kept for an id that comes back naming another series (`known.rs`, 2026-11-23).

## Developing against a real Suwayomi

`apps/leaf/.dev/` (gitignored) holds a local Suwayomi jar and data. Run it with its root dir there
(`-Dsuwayomi.tachidesk.config.server.rootDir=$PWD/data`), then `leaf-server` with
`LEAF_DATA=apps/leaf/.dev/leaf-data`. Never commit `.dev/`.
