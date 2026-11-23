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
  (`/v1/...`, bearer token). Follow searches every source and pins the series to the one you can
  read the most chapters of in a row; every 6 h it refreshes, keeps the next 5 unread chapters
  downloaded, and once a day compares sources, moving a series when yours is down to its last
  few chapters in a row and another has more, or when yours failed three refreshes. AniList gives
  each series its other titles, so a source that names it differently is still found (MangaDex's
  "Junk to King: I Am A Scavenger" is Drake Scans' "Became the King by Scavenging"). Extensions
  update daily. Checked live 2026-11-22: Junk to King moved from MangaDex (chapters 23–44 missing)
  to Drake Scans (all 84) with chapters 1–22 still read and 23–26 already downloading.
- **The phone** (`native/crates/leaf-app`): Connect, Library, Search and Follow, Series (with
  rows for missing chapters), and a continuous vertical reader that chains chapters, caches pages
  on disk and reports progress. The last library and series are kept for offline.

## Building

```fish
cd apps/leaf/native
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -p leaf-app                                  # desktop window
cargo run -p leaf-app -- --shot out.png --screen reader --id 1   # headless shot vs a local leaf-server
cargo run -p leaf-app -- --shot out.png --screen book --books a.epub --seed 1 --at 0.5   # a book (see desktop.rs)
tools/build-lib.sh                                     # libleaf.so (needs ~/.local/share/rust-android + NDK)
cd .. && ./gradlew assembleDebug                       # native/android/build/outputs/apk/debug/app-debug.apk
```

Shots and anything that opens a window: headless only (`--shot`), never on the desktop.

## The server on server1

`~/apps/leaf` holds `native/` (leaf-proto and leaf-server only) and `server/` (`compose.yml`,
`.env` with `LEAF_TOKEN` and `LEAF_SOURCES`). Nothing is published: leaf-server joins the bot's
network as `leaf-server`, and the bot's Caddy routes `/leaf/*` to it (Discord-bot-V2 #44).

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
