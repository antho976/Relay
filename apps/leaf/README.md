# Leaf

Antho's reading app, in Rust drawn with wgpu (Tally RS's renderer), for Android and a desktop
window. Mockups: Relay design 42. **Books** (EPUB, PDF, text) are imported from the phone and read
to be remembered: highlights, notes and a daily review. **Comics** (manhwa, manga and manhua) come
from a server that fetches, follows and keeps them downloaded.

## Books

Books live on the phone: no server, no account. The library's + (or "Add books" on an empty
library) opens the system's file picker for one or many EPUB, PDF and text files; `LeafActivity`
copies each into the app's `inbox/`, and an import thread reads its title, author and cover (the
EPUB's OPF; a text file's own "Title:" and "Author:" lines; a PDF's first page), moves it to
`books/` and adds it to `shelf.db`. A book with no cover gets one drawn from its title. On the
desktop, `LEAF_PICK=/path/a.epub:/path/b.pdf` stands in for the picker.

- **The shelf** (`shelf.rs`): SQLite (rusqlite, bundled) in the data folder: books with their place
  (section, block, byte) and progress, highlights (place range, text, colour, note, review
  schedule), seconds read per local day and book, reviews answered per day, and settings (the type
  sheet). One worker thread owns the connection; the app keeps the whole shelf in memory.
- **The reader** (`book.rs`, `typeset.rs`, `epub.rs`): an EPUB is read once per opening on a
  worker (`zip` + `quick-xml`: spine order, XHTML to headings, paragraphs, quotes, list items,
  pictures, bold and italic spans, links; contents from the EPUB 3 nav, else the NCX, else the
  headings). Text is set in Literata by Leaf itself (fontdue: wrapped at spaces, ragged right),
  a block at a time as it nears the screen, and scrolled as one continuous column on the comics
  strip's rules, so changing the size never jumps. Type sheet: size, spacing, margins, and Paper,
  Sepia, Dark or OLED. Contents with minutes per chapter at your pace; time left in the chapter
  from your own measured speed in that book. A link inside the book (a footnote) opens in place,
  with "Go there". Search in the book, hits grouped by chapter. Text files read the same way.
- **Highlights**: long-press a word, drag to extend (or drag either handle after); the bar offers
  Key idea, Question, Example and Disagree, a note, and Copy. Tap a highlight to recolour, note or
  remove it. **Notes** lists every highlight by book; a book's notes export as Markdown (the share
  sheet; the desktop saves under `exports/`).
- **Review**: a highlight comes back the next day, then as you choose: Sooner (3 days), Normal
  (2 weeks), Later (2 months) or Never.
- **Stats**: minutes read this week by day, against last week, the streak, your pace, highlights
  and reviews. Time counts only while the page moves; a pause over 90 seconds is not reading.
- **PDF**: on the phone, pages are drawn by Android's `PdfRenderer` (`LeafActivity.pdfRender`)
  into the comics strip, a page at a time near the screen. The desktop imports PDFs but says they
  open on the phone: there is no pure-Rust renderer worth bundling.

Literata (`native/crates/leaf-app/assets/fonts/literata_*.ttf`, regular, italic and semibold from
googlefonts/literata) is under the SIL Open Font License, `assets/fonts/Literata-OFL.txt`.

## How comics reach the phone

```
phone (Leaf) ──https──▶ anthobot.bid/leaf ──Cloudflare Tunnel──▶ bot's Caddy ──▶ leaf-server ──▶ Suwayomi ──▶ sources
```

- **Suwayomi** (`ghcr.io/suwayomi/suwayomi-server`) runs Mihon's extensions (one per site:
  Asura Scans, MangaDex, Drake Scans), downloads chapters and converts pages to WebP: a 46.5 MB
  chapter became 6.3 MB. Its WebP quality setting is broken in 2.4 ("No compression type set!"),
  so leaf-server sets the conversion with no quality.
- **leaf-server** (`native/crates/leaf-server`) is the autopilot and the phone's only door
  (`/v1/...`, bearer token; every request is listed at the top of `api.rs`). Follow searches
  every source and pins the series to the one you can read the most chapters of in a row; each
  series is refreshed every 6 h and keeps the chapter after the last one read downloaded, and
  once a day the other sources are compared, moving a series when yours is down to its last few
  chapters in a row and another has more, or when yours failed three refreshes. The hourly pass
  is spread out: at most a sixth of the series refreshed (oldest first) and a 24th compared, so
  ~170 series never become a burst. AniList gives
  each series its other titles, so a source that names it differently is still found (MangaDex's
  "Junk to King: I Am A Scavenger" is Drake Scans' "Became the King by Scavenging"). Extensions
  update daily. Checked live 2026-11-22: Junk to King moved from MangaDex (chapters 23–44 missing)
  to Drake Scans (all 84) with chapters 1–22 still read and 23–26 already downloading.
- **Each source asked politely** (`budget.rs`): everything that reaches a site through Suwayomi
  goes through that source's budget. Background work (the pass, comparisons, recommendations,
  the download line) waits its turn, one request every 3 s per source (MangaDex 1.5 s, AniList
  2 s; `LEAF_SOURCE_GAP_MS`); what the phone waits on (pages, a typed search, a follow, "Wrong
  series?") goes at once and is counted. A 429, "Too Many Requests" or a failed Cloudflare check
  pauses that source's background work for an hour, twice as long on each repeat up to 12 h,
  kept across restarts; Settings' Sources list says so (`/v1/health`: `paused_until`,
  `requests_today`).
- **Downloads** wait in leaf-server's own line and are handed to Suwayomi two per source at a
  time, a minute apart at most: "Download next 5", a range or every unread chapter
  (`POST /v1/series/{id}/download`, `POST`/`DELETE /v1/downloads`) fill in steadily. A single
  chapter tapped goes at once.
- **Chapter pictures** are made on the server too (`thumbs.rs`, the phone's `thumb.rs` scoring
  ported as it is): from up to three pages of a downloaded chapter, read from Suwayomi's download
  folder (never from a site), 240 × 180 WebP in `data/thumbs/`, kept for good; 60 per hourly
  pass and within a minute of the phone asking (`GET /v1/chapters/{id}/thumb`, 404 until then;
  `thumb` on each chapter says one exists).
- **Recommendations** (`recs.rs`, once a day): AniList's recommendations from the series you've
  read, scored (ratings pointing there, more from series you finished, your genres and tags, less
  for tags you asked fewer of), without what you have, hid ("Not interested", kept for good),
  adult titles or non-comics, and only titles a few broad sources carry (searched as background
  work, kept a week): Top picks, Because you read X (your last three), Collections (your four
  most read tags), one finished series to binge (`GET /v1/recs`); "Readers also like" per series
  (`/v1/series/{id}/also`, kept a day, searches no source).
- **Paid on Webtoon** (`webtoon.rs`): Webtoons.com's source lists only free episodes; once a day
  the series' own list page says how many the app sells ahead ("Read 15 new episodes"), and a
  series whose newest is beyond every source's gets `paid` ("Chapters 9–23 are on Webtoon",
  with the link). No paid episode is ever fetched.
- **App updates**: the server keeps the newest APK (`data/app/`, put there by
  `tools/publish-apk.sh <apk> <build> <version>`), `GET /v1/app/latest` and `/v1/app/apk`.
- **The phone** (`native/crates/leaf-app`), three tabs (design 42, pages 12 to 14):
  - **Home** (`home.rs`): a spotlight over a cover that slides gently to the next every 18 s (it
    is not dragged; its dots choose one; a cover too small to fill it sharp is a poster over a
    blurred copy of itself), belled series with new chapters first, then what you read last, books too; Continue reading (covers
    with how far each is read, "Ch. 338 · 2 new"); Recently updated.
  - **Search**: every source at once. **Add** puts a series in My space with its bell off: adding
    is not following. Sources are never named anywhere in the app.
  - **My space** (`space.rs`): Updates (chapters that came over the last 14 days, `/v1/updates`,
    by day, unread in full ink and read dimmed, Belled only or All; a row opens the reader at that
    chapter), Recent (last read first, Continue or Start), Following (webtoon's Subscribed list:
    the bell is the follow, sorted by Recently read, updated or followed, with a row of shelves:
    Reading, Plan to read, On hold, Dropped, Done, and their counts) and Downloads (per series,
    what the server keeps and what is on this phone, and removing it). Updates, Recent and
    Following have a field that narrows the list by title as you type (any case or accent, inside
    words). A long press on a series' row puts it on another shelf; Dropped series stay out of
    Home's spotlight and Continue reading. Highlights and Reading time are in its ⋯ menu, no
    longer tabs; a book's notes stay in the reader's menu.
  - **Series** (`series.rs`): banner, facts, Continue, Chapters / About; Newest ⇅ kept per series.
    Each chapter row has a picture cut from its own pages once downloaded (`thumb.rs`: the most
    colourful 4:3 crop of a few random pages, near-white weighed hard against, kept in `thumbs/`),
    a cloud tile until then that downloads it when tapped, and Read, Continue or a check. ⋯ has
    "Download next 5" and "Remove from My space". Nothing downloads by itself: the server keeps
    only the chapter after the last one read.
  - A long press on a chapter's row: Mark as read or unread, Mark all before as read, Download or
    Remove download, each shown at once and put back with a toast if the server says no. The
    series' ⋯ also has its shelf and "Wrong series?" (the other entries for the work with their
    chapter counts, `/v1/series/{id}/alternatives`; picking one repins the series, place carried).
  - **Notices** (Android): `LeafChaptersJob` asks `/v1/events?since=` about hourly and posts one
    notice per run of new chapters of a belled series; the first bell turned on asks to post
    notifications. When to post is `notices.rs`'s (called through JNI): quiet hours (default
    23:00 to 08:00) hold notices until they end, and "One summary a day" posts a single "12 new
    chapters in 5 series" at its time (default 09:00). Both are in Settings, kept in
    `files/config.json` under `notify`.
  - **Reading comfort** (comics, and books where it applies): a tap in the top or bottom third
    turns the page by 85% of a screen (a setting), as the volume keys do (`LeafActivity` takes
    them while reading: GameActivity never passes them on); the screen stays on while reading; a
    dimmer below the phone's lowest brightness (the reader's overlay, the book's type sheet);
    long white or black runs at a comic page's top and bottom are cut to 16 px when it is decoded
    (`logic::gutter_cuts`, a setting). Comics auto-scroll from the overlay's play key at the
    series' own speed (− and +, kept per series); it waits while the page ahead is loading, a
    tap stops it, and it stops by itself only at the last chapter's end.
  - **Settings** (the gear on Home): the server, then Reading, Notifications, Sources (each one's
    health from `/v1/health`), AniList (a token kept on the server, which pushes chapter progress)
    and Backup (the latest nightly `/v1/backup` to the share sheet, the desktop saves it under
    `exports/`; or one made now).
  - The reader chains chapters, caches pages on disk and reports progress. The last library and
    series are kept for offline.

## Building

```fish
cd apps/leaf/native
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -p leaf-app                                  # desktop window
cargo run -p leaf-app -- --shot out.png --screen reader --id 1   # headless shot vs a local leaf-server
cargo run -p leaf-app -- --shot out.png --screen space --tab following --sort 1   # home, space, series --tab about, search
cargo run -p leaf-app -- --shot out.png --screen book --books a.epub --seed 1 --at 0.5   # a book (see desktop.rs)
tools/build-lib.sh                                     # libleaf.so (needs ~/.local/share/rust-android + NDK)
cd .. && ./gradlew assembleDebug                       # native/android/build/outputs/apk/debug/app-debug.apk
```

Shots and anything that opens a window: headless only (`--shot`), never on the desktop.

## The server on server1

`~/apps/leaf` holds `native/` (leaf-proto and leaf-server only) and `server/` (`compose.yml`,
`.env` with `LEAF_TOKEN` and `LEAF_SOURCES`). Nothing is published: leaf-server joins the bot's
network as `leaf-server`, and the bot's Caddy routes `/leaf/*` to it (Discord-bot-V2 #44).
leaf-server's settings are its environment (the table at the top of `main.rs`); its data folder
(`data/leaf`, `/data` in the container) holds `leaf.db`, `backups/`, `thumbs/`, `recs/covers/`
and `app/`.

```fish
apps/leaf/tools/publish-apk.sh native/android/build/outputs/apk/release/app-release.apk 12 0.3.0
```

puts a new app there (`app/leaf.apk` and `app/latest.json`, copied under a temporary name and
renamed, so the phone never sees one without the other).

```fish
ssh server1
cd ~/apps/leaf/server
docker compose up -d --build        # after copying a new native/ over
docker compose logs -f leaf-server
cat .env                            # the token the phone needs
```

Until leaf-server is on main with an image of its own, a new version is copied over by hand
(`tar` of `native/` without `target/`).

## Not done yet

- Books: the Book page (design 42 page 2), recaps, look-up and "ask", syncing through the server,
  reading time for comics and PDFs (only EPUB and text count), and full-text search across every
  book (Find searches titles, authors and your notes).
- The phone does not yet download chapters ahead on its own while closed; it prefetches while
  reading.
- leaf-server is not built by CI or published as an image.
- The phone does not read the server's round-3 answers yet: recommendations (`/v1/recs`,
  `/also`), chapter pictures made on the server (`thumb`), paid episodes (`paid`), batch
  downloads and app updates (`/v1/app/latest`) are served, not shown.
