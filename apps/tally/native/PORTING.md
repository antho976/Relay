# Porting a screen to Tally RS

> **The look is now "Premium, dark and light"** (2026-11-19, the owner's call, drawn as Relay design 41
> "Tally: premium (Wealthsimple feel), dark + light"). It replaces Direction C's sky. Read the
> section "The premium look" at the end before touching a screen; it wins over anything older here.

How every screen is written, so they all read and behave alike. Read `README.md` first, then
`crates/tally-rs/src/overview.rs` and `entry.rs`, the two screens done the house way.

## The shape of a screen module

```rust
#[derive(Default)]
pub struct State { /* what the module keeps between frames: caches, open sheets, drafts */ }

pub fn draw(app: &mut App, dt: f32) -> bool {
    // A module with several routes matches app.route() first and calls one fn per page.
    let pg = app.page_begin(dt);             // scroll, and where content starts (pg.top)
    if app.fresh { /* first frame on this page: (re)load state from app.store */ }
    let mut act = None;                       // what was tapped, acted on after drawing
    let bottom;
    {
        let mut cx = cx!(app);               // canvas, finger, palette, focus, host
        let st = &mut app.st.<module>;        // disjoint borrow: fine beside cx
        // draw top to bottom with a running y; every control returns whether it was tapped
        bottom = y;
    }
    app.page_end(&pg, bottom);
    match act { /* app.push(..), app.pop(), app.replace(..), writes then app.invalidate(), app.say(..) */ }
    pg.moving                                 // or true while waiting on something (a reply)
}
```

- **Routes** are in `nav.rs`. Space pages (`is_space()`) get the fixed top row and the bottom bar
  from `app.rs` and start under them; their first thing is `app::page_title(cx, y, title, context)`.
  Pushed pages start with `widgets::page_header(cx, y, title, action)` (back key, title, an ink
  action such as "Save").
- **Never block the UI thread**: no network, no file walk, no big parse in `draw`. Read the ledger
  when `app.fresh` or when `app.ledger_gen` moved since you cached (keep the gen you read at).
- **Taps**: `cx.tap(x, y, w, h)` in content units (it applies the scroll). A tap is taken once.
  Sheets and dialogs: `let held = widgets::hold(&mut cx)` before drawing the page, then
  `widgets::release(&mut cx, held)` and draw the sheet last, so the sheet gets the tap.
- **Text fields**: `widgets::field` (boxed, labelled) and `widgets::inline`, each with a unique
  `&'static str` id like `"plan.bill.name"`. The keyboard and Done/back are handled for you;
  `app.page_end` keeps the focused field above the keyboard. Amounts: `numeric: true`, then
  `widgets::parse_minor(text, fmt.fraction_digits)`; `widgets::minor_text` to fill one.

## Look and words

- Components: `kit.rs` (glass, postcard, gauge, tint tiles, meter, panel, keys, round keys, badges,
  date badges, donut) and `widgets.rs` (rows, toggles, segmented, pills, fields, sheets, calendar,
  confirm, empty). Colours only from `cx.p` (the palette) and `theme::hue` / `theme::category_color`.
  Type sizes from `kit::ty`. Never hard-code a colour a palette has.
- Match the Kotlin screen (its `ui/<area>/*.kt`, and the reference captures in
  `app/src/test/screenshots/`; the Kotlin app left the repo after `cf4b63b`, so read them with
  `git show cf4b63b:app/...`): the same sections, the same words, the same order. Its copy rules
  (`relay_money::copy`, `PRODUCT.md`'s voice: dry, specific, no exclamation marks, no em dashes)
  hold. Honest at zero: every empty list draws `widgets::empty` with what to do.
- Confirm only what cannot be undone (`widgets::confirm`, naming the act); everything else is Undo
  in a toast.
- If you need a control the kit lacks, write it in your own module (or `<module>_kit.rs`), in the
  kit's style. Do not edit files you were not given.

## The ledger

- Read and write through `app.store.ledger` (relay-money) whenever it has the call: `summary`,
  `tx_list`, `series`, `accounts`, `categories`, `lists`, `tx_add`/`tx_update`/`tx_delete`/
  `tx_restore`, `budget_set`, `account_add`/`account_update`, `invest_*`, `room_set`,
  `value_set`, `post_due`, `export_backup`/`import_backup`, `set_month_start_day`, `set_currency`.
- What it has no call for yet (bills in `recurring`, `goals`, `contributions`, `categories`
  edits): write through `app.store.sql()` (a rusqlite connection to the same file). Set `uid`
  (`Store::stamp()`), `updated_at` (ms), never touch `seq` (triggers number it for sync); delete by
  setting `deleted = 1` and `updated_at`. Read the schema in relay-money's `ledger.rs` first and
  mirror its conventions; put such calls in your module's own `*_store.rs`, with tests.
- Money is `i64` minor units; format with `app.store.formatter()` (`format`, `format_whole`).
  Dates are `relay_money::Date` (jiff civil dates), stored as `YYYY-MM-DD`.
- After a write: `app.invalidate()`.

## Checking your work

```
cd native
cargo build                                   # warnings count: leave none
cargo test
./target/debug/tally --shot /tmp/x.png --screen <name> [--scroll N]
```

Add your screen names to the `--screen` match in `desktop.rs` only if you were given that file;
otherwise shoot through an existing name and navigate in code. Look at every screen you made at
real size, beside the Kotlin capture, and fix what differs before you report.


## The premium look

The owner asked for Tally to feel like Wealthsimple: premium, thought through, in Light and Dark.
Overview (`overview.rs`) is done the house way in this look; read it first.

**Ground and colour.** Pure black (Dark) or white (Light) behind everything, no gradient. Cards are
`kit::card` (the palette's `slab` with a hairline `line_subtle` edge, radius 24). Keys and chips sit
on `card2`. Ink, `ink2`, `ink3` for words. One accent, brass (`p.signal`): today's mark, "in 3
days", the focused field. State colours only for state: `live_text` (good, income), `held_text`
(over, errors), `waiting_text` (warnings). Category colours only through `theme::category_color`
(it gives the Dark or Light shade). Never write a hex colour a palette or the theme has.

**Type.** Geist. Page titles 17 semibold, centred in the bar. Big figures with `kit::money`
(cents smaller and raised). Section headers `kit::section` (20 semibold, "See all" on the right).
Eyebrows 12 semibold upper, `ink3`. Rows: 16 medium title, 13 `ink2` line.

**Pieces** (`kit.rs`, `widgets.rs`): `card`, `card_button`, `pic_card` (a picture card: picture
fills it, words on its plain lower part; `PicCard { eyebrow, title, title_size, line, meter,
button }`), `picture` (a picture in any box), `section`, `money`, `entry_row` (64 tall),
`badge` (rounded square, category tint), `meter` (thin, with a pace mark), `date_badge`,
`carousel`/`carousel_end` (a sideways row with an `ui::HScroll` in your State), `key`
(`Primary` is the one filled action), `round_key` (glyph only), `widgets::page_header` (back
arrow, centred title, and an action: a glyph name like `"plus"` draws a key, words like `"Save"`
a small filled pill), `segmented`, `pills` (chips), `field`, `row` (a glyph tile, title, line,
trailing, chevron), `toggle_row`, `empty`, sheets and `confirm`.

**Pictures** (`pictures.rs`): studio renders, a light one for Light and a dark one for Dark, picked
for you by `kit::picture`/`kit::pic_card` from the plain name. Names: wallet, piggy (budgets),
receipts (bills), cards (accounts), chart (investments), bank, suitcase, house, umbrella, carkey,
rings, laptop, school, baby, pet, retire, fitness, gift, renovate, debt, coin, hero.
`pictures::for_goal(name, invest)` picks a goal's. Words on a picture use `theme::pic::*`, which
`pic_card` does for you.

**Navigation.** The tabs are Overview, Entries, Insights and Search, with + (a new entry) in the
middle; their pages get the top bar (drawer key, title, bell for bills, the person key for
Settings) and start at `pg.top`. Everything else (Budgets, Bills, Goals, Accounts, Investments,
Categories, Settings and the rest) is pushed: start it with `widgets::page_header(cx, pg.top, ...)`
and Back pops it. The drawer (swipe right on a tab's page, or the menu key) reaches every page.
A row that scrolls sideways must use `kit::carousel` so its swipes are not taken for the drawer.

**Rules the owner set for this look** (do not undo): Overview has no quick-action row and Goals is
its last section; Insights puts the pace chart first and the donut shares one card with its
category list; Goals is one featured card for the closest goal, then a two-column grid of smaller
picture cards and a dashed "New goal" tile; Investments uses the glass-bars picture (never a plant);
the + is the tab bar's middle key.

**Reference.** Each designed screen, Dark beside Light, is a PNG in the session's scratchpad
`ref/` folder (the brief names it); the canvas is design 41. Match its order, sizes and words, and
keep the screen's real features (the Kotlin parity) where the mockup drew fewer.

**Checking.** `./target/debug/tally --shot out.png --screen <name> --palette dark` (and `light`);
look at both, at the top and scrolled (`--scroll N`).
