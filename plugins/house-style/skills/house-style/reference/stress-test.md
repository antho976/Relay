# Stress test: break it with worst-case data

UI that looks right with demo data was built against kind data: "Jane Doe, jane@acme.com, 12
members". Find the realistic worst case for every value a screen renders, put both datasets side by
side, and report what broke. Be the most annoying real user it will meet: Aleksandra
Wiśniewska-Kowalczyk, with a colleague at
`bartholomew.fitzgerald@northwind-industries-holdings.example.com`, an intern called Jo, and a
workspace of 1,284 members. All of them exist in production somewhere.

**Plausible or schema-backed, never random.** `"aaaaaaaa…"` and 5,000-character names prove nothing.
Use a real naming pattern or the actual limit from the schema, column or API. No limit found is
itself a finding ("unbounded"). **Don't stop at long text:** the breaks that ship are "1 members",
the missing avatar, the empty list, the orphaned dash, the translated badge.

## Workflow

1. **Map every rendered value:** `Field | Source | Type | Limit | Optional?`, including the ones
   people forget (counts in headers, relative times, badge and status text, labels from data,
   tooltips, avatars, and the list's own length). Find limits in validation schemas, migrations, API
   types and `maxLength`. A 50-character input saving to a 255-character column means something
   longer arrives eventually, through an import or the API.
2. **Build one worst-case fixture** beside the demo data, shaped exactly like it, mixing failures
   across the first rows (row 1 the long name, row 2 the long email, row 3 the one-letter name), as
   real data does. Add the states one fixture can't hold: empty, one item (every count at 1), and the
   realistic maximum (1,000+ rows if unpaginated).
3. **Change the data, not the component.** The worst case enters where the demo data does (fixture,
   mock, props, API stub, a Room seed, a `@Preview` parameter). Hand-editing markup tests your edit.
4. **Make it flippable**, dev-only and never shipped:
   - *Web:* a **Demo / Worst case / Empty / One / 1,000 rows** segmented control fixed bottom-centre,
     plain grey chrome, switching instantly, kept in the URL (`?data=worst`). With no project, one
     self-contained HTML file.
   - *Compose:* a `PreviewParameterProvider` with each fixture, plus Roborazzi goldens at 100% and
     200% font scale.
   - *GTK:* a debug env var that seeds the fixture, captured in the virtual compositor
     (`house-style-native`).
   - *Design canvas:* the worst-case copy on a duplicate artboard beside the original.
5. **Break it** at the component's real container width, then the narrowest (320 px web, the
   minimum window on desktop) and the widest; at 200% text; in dark mode and RTL if supported.
   Screenshot each state. Say which findings you saw and which you inferred from code.
6. **Report and stop.** Some breaks are design decisions. Fix only when asked (`fix all`,
   `fix 1, 3`), then flip through every state again (the demo too) and keep the fixture as the
   regression test.

## Failure signatures

| What you see | Cause | Fix (web; same idea on native) |
| --- | --- | --- |
| Avatar or icon squished into an oval | Fixed-size child shrinking | `flex-shrink: 0` |
| Text overflows instead of wrapping or truncating | Flex/grid child at `min-width: auto` | `min-width: 0` (`minmax(0, 1fr)`); Compose `Modifier.weight(1f)` |
| Email or URL runs past the edge | No break opportunities | `overflow-wrap: anywhere` |
| Trailing action pushed off or clipped | Middle content took the space | `min-width: 0` on the middle, `flex-shrink: 0` on the action |
| Badge wraps to two lines | Badge allowed to shrink | `white-space: nowrap; flex-shrink: 0`, and decide what yields instead |
| Avatar adrift beside a three-line name | Centred alignment on rows of varying height | Top-align once text can wrap |
| Last row cut mid-glyph | Fixed height, no scroll affordance | A visible scroll or fade, or hug the content |
| Heading breaks mid-word | `word-break: break-all` | `overflow-wrap: anywhere` |
| Wrong initials ("J", "CI", "�") | `.split(' ')[0][0]` | Grapheme clusters (`Intl.Segmenter`), first and last word, fallback icon |
| Orphaned dash or empty line | Placeholder for a missing optional field | Omit the line, or reserve it deliberately |
| "1 members", "0 member" | Hard-coded plural | `Intl.PluralRules`; Android plurals resources |
| Numbers jitter or misalign | Proportional figures | Tabular numerals |
| `1284`, `0.30000000000000004`, `NaN`, `undefined` | Raw numbers | Locale number formatting; guard nulls |
| Translated button label overflows | Fixed width | Width from content, with a minimum |
| Vietnamese or Thai diacritics clipped | Tight line height with clipping | Looser line height, no clipping on text |
| Broken-image icon | No error fallback | Initials fallback; cover-fit any aspect ratio |
| Truncated text with no way to read it | Ellipsis alone | Tooltip or `title`, and the full value in a detail view |
| 1,000 rows stutter | Every row rendered | Virtualise (`LazyColumn`, `GtkListView`, Virtuoso) or paginate |
| Raw `<b>`, `&amp;` or `**text**` | Wrong escaping layer | Escape once, at render; never inject user data as HTML |

**Truncate, wrap or clamp, per field.** Wrap what identifies something (names, titles in a detail
view; two lines is fine, four means the column is too narrow). Truncate at the end for secondary
metadata, always with a way to see the whole. Truncate in the middle where items differ at the end
(file names, shared-domain emails, paths, hashes, session names). Clamp multi-line previews in cards.
Never truncate numbers, amounts, dates or anything compared.

## Worst-case catalog

Use `example.com`, `example.org` or `.test` domains so a fixture never points at a real inbox.

**Names:** `Aleksandra Wiśniewska-Kowalczyk` (long, hyphen, diacritics) · `Christopher Alexander
Montgomery III` (suffix: naive initials give "CI") · `Jo`, `J` (tiny, single-letter initials, short
click target) · `Ólafur Darri Ólafsson` (accented capital, sorting) · `Đặng Thị Ngọc Hân` (stacked
diacritics) · `王秀英` (CJK, no spaces) · `نور الهدى عبد الرحمن` (RTL, needs `dir="auto"`) ·
`Seán O'Brien-Ó Súilleabháin` (apostrophe: escaping, search) · `María José de la Cruz y Fernández`
(particles, "last name" sorting) · `dana` (lowercase, initials still upper) · `🦊 Fox` (emoji first:
half a surrogate pair) · `👩🏽‍💻 Priya` (ZWJ sequence, `.length` 7+) · `  Sam   Lee ` (stray spaces) ·
missing name, email only.

**Unbreakable strings:** the long email above · `a@b.co` · `first.last+billing-notifications@example.com`
(plus addressing) · `ops@sub.department.region.example.co.uk` · a long URL with a query string ·
a UUID · `Q3 Board Deck — FINAL (revised) v12 [approved by legal].pdf` ·
`IMG_20250914_183022_HDR_portrait_edited_edited.HEIC` · `@a` / a 36-character handle.

**Labels from data:** `Senior Product Design Engineer, Platform Infrastructure` ·
`Invitation expired 12 days ago` as a badge · `Benachrichtigungseinstellungen` (30 characters, no
spaces) · `Paramètres de confidentialité et de sécurité` (translations run about 30% longer) · twelve
tags on one item (needs "+8") · one 45-character tag · `Untitled`, empty or whitespace-only titles ·
`<script>alert(1)</script>`, `&amp;`, `**bold**` · a newline in a one-line field · a 2,000-character
pasted description.

**Numbers:** `0` (zero states, division by zero) · `1` (plurals) · `1284` (separators) · `1000000`
(badge width, compact form) · `12345678.9` as currency · `-42.5` (sign, colour, accounting
parentheses) · `142%` and `-3%` (meters past their bounds) · `null`, `NaN` · `de-DE` vs `en-US`
formatting · a live value crossing `99 → 100`.

**Collections:** 0 · 1 (a grid of one, "1 of 1") · exactly a page and a page plus one (an empty page
2) · 1,000+ unpaginated · one item 10× the size of the others · identical names.

**Time:** now ("0 seconds ago") · 12 days, 11 months and 3 years ago (switch to an absolute date
after about a week) · the future ("in 3 days", not "-3 days ago") · `1970-01-01` · 23:30 local on
31 December, which is a different day in UTC · `1,284 hours` (never rolled up). Use the platform's
relative and absolute date formatters.

**Media:** an avatar that 404s · no avatar · a 4000×200 panorama and a 200×4000 strip · a
transparent or dark logo on dark mode · a slow image (layout shift without fixed dimensions).

**States:** loading (a skeleton that doesn't match the final layout) · an API error (a raw
`TypeError` on screen) · partial data across a list · every status value at once · no permission ·
the current user in the list ("You", actions that don't apply to yourself).

**Environment:** 320 px, a 280 px sidebar, 2560 px, 200% text, dark mode, RTL, and touch (actions
that only appear on hover can't be reached).

## Report

Part 1, what broke, worst first: `# | Severity | Field | Worst-case value | What happens | Fix`,
with `file:line` for each fix. **Broken** means unreadable, unreachable or wrong data. **Ugly** means
readable but visibly wrong. **Fragile** means fine now but one realistic step from breaking (no
limit, no fallback). Part 2, decisions for the owner (truncate or wrap, what an empty role shows,
paginate or virtualise), each with a recommendation. Part 3, what held up, so nobody touches it.
Close with where the toggle is and its states. Keep it matter-of-fact: the component isn't badly
built, it was built against kind data.
