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
- **The server's downloads** (plumbing, never shown in the app as downloads) wait in
  leaf-server's own line and are handed to Suwayomi two per source at a time, a minute apart at
  most (`POST`/`DELETE /v1/downloads`, `POST /v1/series/{id}/download`). The phone's download line
  puts a chapter there before fetching its pages, so they come from the server's disk.
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
  most read tags, each with its picture from `LEAF_TAGART`, `GET /v1/recs/tagart/{slug}`), one finished series to binge, which the phone no longer shows (`GET /v1/recs`); "Readers also like" per series
  (`/v1/series/{id}/also`, kept a day, searches no source).
- **Paid on Webtoon** (`webtoon.rs`): Webtoons.com's source lists only free episodes; once a day
  the series' own list page says how many the app sells ahead ("Read 15 new episodes"), and a
  series whose newest is beyond every source's gets `paid` ("Chapters 9–23 are on Webtoon",
  with the link). No paid episode is ever fetched.
- **App updates**: the server keeps the newest APK (`data/app/`, put there by
  `tools/publish-apk.sh <apk> <build> <version>`), `GET /v1/app/latest` and `/v1/app/apk`.
- **The phone** (`native/crates/leaf-app`), three tabs (design 42, pages 12 to 14):
  - **Home** (`home.rs`, design 42 page 15): the spotlight, a rounded card of big art (only covers
    that fill it sharp) with the title centred, Continue or Start and the bell as Follow; it slides
    gently to the next every 18 s (it is not dragged; its dots under it choose one); belled series
    with new chapters first, then what you read last, books too. Then **Top picks for you** (three
    across, each with why in the accent), **Continue reading** (square covers, "1 new chapter" in
    the accent or "Ch. 12 · 58 to go": what you started and haven't finished, last read first, a
    belled series' new chapter bringing it back to the front; finished ones only fill it up to 15,
    after the others, "Read" dimmed; `logic::continue_order`; the spotlight and My space's Recent
    put finished ones last too), **Because you read X** rails, **Collections** (a tile per
    tag, each opening its list). Each Because rail sits under a banner, a still collage of its
    covers; a rail of fewer than two picks is left out, and the server's finished series to binge
    is not shown. A collection's tile is 16:9: the tag's picture (`/v1/recs/tagart/{slug}`, kept on
    disk; a 404 asked again a day later), else a still collage of its covers, the name over a dark
    foot. Picks (`recs.rs`) come from `/v1/recs`, kept on disk and asked again at most every 30
    minutes; until the server has made them the section says so. A tap on a pick opens its page as
    a preview (see Search); a long press opens its sheet: why it is there, Follow (added, bell on),
    Read chapter 1 (added, then opened), Not interested (hidden for good) and Fewer like a tag.
  - **Search**: a few broad sources (`LEAF_SEARCH_SOURCES`, default MangaDex, Asura Scans, Weeb
    Central, MangaFire), each asked apart (`/v1/search?source=`) so results show as they come; only
    titles with every word searched (or the same work, `leaf_proto::title_has_words`), at most 20,
    each source's answer kept 10 minutes on the server; a word may be a letter off (two in words of
    six letters or more). It goes once typing pauses (700 ms) or on Enter, never under 3
    characters, never twice for the same words; "Search all sources" under the results asks the
    rest. When nothing is found, AniList is asked once what the words meant
    (`/v1/search/suggest`, kept a day): "Did you mean The Awakening Contract?" searches that. Text
    the keyboard sends after Enter let the field go goes back to it. A row opens the title's page:
    a series added already as it is, any other as a **preview** (`/v1/preview`, under a negative
    id): its chapters, Follow and a download key beside it (Select chapters, Select range…, the
    next 5, every unread one), nothing added. Reading a chapter, Follow or a download adds it
    (`/v1/follow` with its entries), and the page becomes the series'. Sources are never named
    anywhere in the app.
  - **My space** (`space.rs`, design 42 page 16): rows with a square cover flush left (88, smaller
    than drawn, so more fit), the title, a line, when, and a key. Updates (chapters that came over
    the last 14 days, `/v1/updates`, by day, unread in full ink and read dimmed, Belled only or All;
    a row opens the reader at that chapter), Recent (last read first, Continue or Start),
    Following (webtoon's Subscribed list: the bell is the follow, sorted by Recently read, updated
    or followed, with a row of shelves: Reading, Plan to read, On hold, Dropped, Done, and their
    counts) and Downloads (see **Downloads** below). Updates,
    Recent and Following have a field that narrows the list by title as you type (any case or
    accent, inside words). A long press on a series' row puts it on another shelf; Dropped series
    stay out of Home's spotlight and Continue reading. Highlights and Reading time are in its ⋯
    menu, no longer tabs; a book's notes stay in the reader's menu.
  - **My space holds** only series with a chapter read, the bell on or a chapter downloaded
    (`logic::in_space`); one added and left alone shows in none of its tabs, nor on Home.
  - **Series** (`series.rs`, design 42 page 17): Back, the bell as a Follow pill, the download key
    (Select chapters, Select range…, the next 5, every unread one), info and ⋯; the
    cover centred over a wash of its colours; title, authors, three lines of synopsis (info shows
    it all with the facts), tags; tabs **Chapters** / **Readers also like**; a fixed key at the
    bottom, "Continue chapter N". Chapter rows have the server's picture of the chapter (72 × 54,
    `/v1/chapters/{id}/thumb`, kept on disk; `thumb` on each chapter says one exists; the phone no
    longer cuts pictures from pages) with a small download key on it until the chapter is on this
    phone (a tap downloads it; a check once it is), "Ch. 339 · title", the date or the page you
    are on, what the phone holds ("Saved", "Saving · 12 of 40 pages", "Waiting", "Didn't save ·
    Retry"), and Read or Continue; read rows are dimmed. Offline with chapters downloaded, the list
    is those alone, under an "Offline" note. When Webtoon sells episodes ahead (`paid`), a row at the newest end says "Chapters
    9–23 are on Webtoon (paid)" and Open hands its link to the Webtoon app or the browser.
    Readers also like is `/v1/series/{id}/also`, three across, "Yours" on ones in My space.
  - **Select** (a long press on a chapter, or ⋯ → Select chapters): taps add or take chapters away,
    and a long press dragged without lifting sweeps every chapter the finger passes (`sweep.rs`:
    the range from where it began to the finger, dragging back lets go, starting on a selected row
    de-selects, the list scrolls by itself near its top and bottom edges, a haptic tick per row;
    Downloads and a series' downloads select the same way),
    and the bottom key becomes Download N / Delete N / Mark read (Delete removes this phone's
    copies only); "Select range…" takes a first and last chapter to select or download; ⋯ also
    has "Download next 5" and "Download all unread". ⋯ also has the shelf, "Wrong
    series?" (the other entries for the work with their chapter counts,
    `/v1/series/{id}/alternatives`; picking one repins the series, place carried) and "Remove from
    My space".
  - **App updates** (`update.rs`): the app knows its build (`LEAF_BUILD`, the commit count
    `tools/build-lib.sh` builds at; Gradle's `versionCode` is the same number, `versionName`
    "0.<build>"). When it opens and whenever it comes back to the front, at most every 15
    minutes, it asks `/v1/app/latest`; a newer build is
    offered in a sheet, "Leaf 0.N is ready · 18 MB", Update or Later (Later hides that build for a
    day). Update downloads `/v1/app/apk` into the cache on a thread, checks its SHA-256, and hands
    it to Android's installer through `LeafActivity` (a FileProvider link); when Leaf may not
    install apps yet, Android's "Install unknown apps" page opens first. The desktop shows the
    sheet; its Update only logs.
  - **Notices** (Android): `LeafChaptersJob` asks `/v1/events?since=` about hourly and posts one
    notice per run of new chapters of a belled series; the first bell turned on asks to post
    notifications. When to post is `notices.rs`'s (called through JNI): quiet hours (default
    23:00 to 08:00) hold notices until they end, and "One summary a day" posts a single "12 new
    chapters in 5 series" at its time (default 09:00). Both are in Settings, kept in
    `files/config.json` under `notify`.
  - **Reading comfort** (comics, and books where it applies): a tap only shows or hides the
    overlay (no tap zones, no volume keys: the owner, 2026-11-23); the screen stays on while
    reading; a
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
  - **Ids** (`known.rs`): the phone remembers which series each id named (`known.json`); an id
    that comes back naming another series lets go of the old one's cover, banner, spotlight
    pictures, place, copy on disk and settings, and its downloads move under an id of their own.
    leaf-server never gives an id twice (`work_next_id`).
  - **Downloads** (`offline.rs`, `saved.rs`): a download is a chapter kept on this phone to read
    with no connection; what the server keeps is its own business and never shown. Pages go to
    `offline/<chapter>/` (a promo page the server skips as `<n>.skip`) with `manifest.json`
    written last (series, chapter, pages, bytes, when), apart from the page cache, whose cleanup
    never touches them; the reader reads them first and asks the server nothing, length included.
    The line (`offline/queue.json`, kept across restarts) goes one chapter at a time, pages in
    order, a pause between chapters, and takes a half-done chapter up where it stopped. A chapter
    the server hasn't downloaded is first put in the server's own line (`POST /v1/downloads`, with
    the next four), then fetched from the server's disk once `/v1/series/{id}` says it is there
    (asked every 15 s, given up after 20 min: "Didn't save · Retry"); out of reach, the line rests
    a minute. My space › Downloads lists, from the folder alone (no server needed), each series'
    cover, "12 chapters · 84 MB" and the newest, the total on this phone, and a line while chapters
    are being saved; a row opens that series' downloads (Read each, offline). Select (a long press,
    Select or ⋯) picks series and the bar deletes them or only their read chapters, after a
    confirm with the count and the room freed; ⋯ → "Delete read chapters" does it everywhere; a
    series' downloads page selects chapters the same way. Deleting never asks the server.
  - **Offline** (`online.rs`): every live answer says whether the server could be reached (no
    answer at all, or a proxy's gateway error with none of the server's words). Once per spell out
    of reach, a sheet over the page (never over a strip or book being read) says "You're offline"
    with a cloud struck through and "Your downloads still open: 15 chapters on this phone", Open
    downloads (My space › Downloads) and Not now; with nothing downloaded it says so and offers OK.
    The server is tried again every 20 s (`/v1/status`) and on coming back to the front; when it
    answers, "Back online" shows as a toast and the library is asked again.
  - **The exact place** (`places.rs`): chapter, page and how far down it, kept in `places.json` on
    each page change and at most every 2 s while the strip moves (a thread writes it), and at once
    when the app pauses, stops, loses focus or the reader closes (progress to the server then too;
    otherwise debounced 3 s). Continue and a chapter's row open there to the unit when it is not
    older than the server's last read; the overlay says "Saved · page 12" with a mark on its track.
  - **Scanlator promo pages** (`leaf-server/src/promo.rs`): pages near a chapter's start (3) and
    end (4) passing through the server get a 64-bit dHash; a picture seen near the edges of 3
    chapters (Hamming ≤ 6), or like one hidden by hand, is answered `204` + `x-leaf-skip: 1` and
    left out of the strip (the place never jumps; a chapter whose last pages are left out still
    finishes). A long press on a page offers "Hide this page and ones like it" (`/v1/pages/hide`,
    Undo for 5 s). Settings: "Skip scanlator promo pages" (off sends `?promo=1`) and "N pages
    hidden by you · Clear". No source is ever asked anything for this.

## Building

```fish
cd apps/leaf/native
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -p leaf-app                                  # desktop window
cargo run -p leaf-app -- --shot out.png --screen reader --id 1   # headless shot vs a local leaf-server
cargo run -p leaf-app -- --shot out.png --screen space --tab following --sort 1   # home, space, series --tab also, picks, search
cargo run -p leaf-app -- --shot out.png --screen book --books a.epub --seed 1 --at 0.5   # a book (see desktop.rs)
cargo run -p leaf-app -- --shot out.png --screen series --id 3 --scroll 600 --sweep 4  # a sweep, the finger still down
cargo run -p leaf-app -- --shot out.png --screen home --data DIR --server http://127.0.0.1:9   # the offline prompt (DIR from an earlier shot)
tools/build-lib.sh                                     # libleaf.so (needs ~/.local/share/rust-android + NDK), LEAF_BUILD = commit count
cd .. && ./gradlew assembleDebug                       # native/android/build/outputs/apk/debug/app-debug.apk, versionCode = the same count
```

Shots and anything that opens a window: headless only (`--shot`), never on the desktop.

## The server on server1

`~/apps/leaf` holds `native/` (leaf-proto and leaf-server only) and `server/` (`compose.yml`,
`.env` with `LEAF_TOKEN` and `LEAF_SOURCES`). Nothing is published: leaf-server joins the bot's
network as `leaf-server`, and the bot's Caddy routes `/leaf/*` to it (Discord-bot-V2 #44).
leaf-server's settings are its environment (the table at the top of `main.rs`); its data folder
(`data/leaf`, `/data` in the container) holds `leaf.db`, `backups/`, `thumbs/`, `recs/covers/`,
`tagart/` and `app/`.

```fish
apps/leaf/tools/publish-apk.sh native/android/build/outputs/apk/release/app-release.apk 12 0.3.0
```

puts a new app there (`app/leaf.apk` and `app/latest.json`, copied under a temporary name and
renamed, so the phone never sees one without the other).

The collections' pictures (one PNG per tag, `<slug>.png`) become 640 × 360 WebP with
`apps/leaf/tools/tagart.sh <src_dir>` (into `apps/leaf/server/tagart/`), and are copied into the
data folder's `tagart/` (`LEAF_TAGART`, default `<LEAF_DATA>/tagart`, `/data/tagart` in the
container):

```fish
ssh server1 'mkdir -p apps/leaf/server/data/leaf/tagart'
scp apps/leaf/server/tagart/*.webp server1:apps/leaf/server/data/leaf/tagart/
```

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
- The phone's download line runs only while the app is open (no background job yet); it prefetches while
  reading.
- leaf-server is not built by CI or published as an image.
- `paid` and `thumb` are read by the phone beside leaf-proto's `Series` and `Chapter`
  (`net::Extras`): leaf-server builds those structs field by field, so they gain the fields only
  with a change on both sides.
