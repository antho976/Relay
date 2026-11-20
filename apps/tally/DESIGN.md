# Design

Tally wears Relay's phone app (antho976/RelayV4 `apps/relay-android`, from Relay's DESIGN.md):

- **Ground and surfaces**: `wall` #171614 (matte; dark #131211; pure black #000000), `slab` cards
  with a 1dp `edge`, `raised` fields with a `strong` stroke, `wash` for pressed rows.
- **Ink**: `ink` #EDE9E2 for what matters, `ink2` for what supports it, `ink3` for captions.
  Primary keys are ink with ground text, one per surface.
- **Colour means something**: `live` green (money in, synced, on pace), `waiting` amber (over pace,
  waiting), `held` red (over budget, a refusal; words in `heldText`), Relay's `signal` green for the
  mark's slash and the pairing code. Category hues (twelve, `ui/theme/Color.kt`) are data, used on
  badges and charts, never on text.
- **Type**: Geist for the interface, Geist Mono for the status bar and small figures, Sora for the
  wordmark; amounts take tabular numerals; one large figure (`hero`) per page.
- **Shapes**: keys 10dp, cards 14dp, pills round, badges 9dp; touch targets 44dp at least.
- **Frame**: the top bar (drawer key, the Tally mark and wordmark, the Spend | Plan switch, search,
  settings), the page in a readable column (720dp at most), the floating Add key, and the status bar
  (the sync lamp and line; what is left and the days).
- **Glyphs**: Relay's set (`ui/kit/Glyphs.kt`, copied, never edited) and a money set drawn on the
  same 16-unit grid in the same hand (`ui/kit/Icon.kt`). Category icons are Material Rounded,
  because their names are the ledger's synced vocabulary.

## Premium, dark and light (chosen 2026-11-19; Tally RS first)

The owner asked for Tally to feel like Wealthsimple: premium, "like more thought went into it", in
white and dark. Drawn as Relay design 41 ("Tally: premium (Wealthsimple feel), dark + light"),
approved with changes, and built in Tally RS (`native/`). It replaces Direction C below there; the
Kotlin app keeps C until it retires.

- **Ground and colour.** Pure black (Dark) or white (Light), no gradient. Cards warm grey with a
  hairline edge, radius 24 (picture cards 28). One accent, brass (#D8B67C on Dark, #8A6634 on
  Light). Category colours have a Dark and a Light shade. Appearance is System, Light or Dark;
  System is the default and follows the phone.
- **Pictures.** Studio 3D renders made on the owner's PC (Qwen-Image): one object floating in the
  upper part, plain ground below for the words. Every picture has a Dark version, made by editing
  the Light one so it is the same object on charcoal with a warm rim light; on Dark the cards use
  it and the words turn light. The owner rejected a potted plant (Investments uses glass bars) and
  a long stack of goal cards.
- **Navigation.** Tabs: Overview, Entries, + (new entry), Insights, Search; icons only. A drawer you
  swipe right to open (or the menu key) lists every page by section with a live reading. A page
  it opens shows the drawer key; everything else is pushed with a back arrow.
- **Overview.** The month on a dark card that stays dark on Light, with the wallet picture; This
  week and Next bill; Recent; Budgets; Goals last as swipeable picture cards. No quick-action row
  (the owner removed Expense, Income, Bill and Transfer from it: the + covers adding).
- **Insights.** The pace chart first, then the donut and its categories in one card.
- **Goals.** One featured card (the closest goal), then a two-column grid and a "New goal" tile.
  A goal's picture comes from its name (trip, house, car, wedding, laptop, school, baby, pet,
  retirement, fitness, gifts, renovation, debt; the piggy bank otherwise).
- **From the owner's first test (0.3.1):** every list sits in a card (Recent, Budgets, Entries' days,
  Bills' groups, Largest); pages stop at their ends (no pulling past the top); picture cards do not
  light up when touched and keep a soft shade of their own ground under their words; a page opened
  from the drawer shows the drawer key, and the same page opened from another page (Overview's
  "All", the bell, Recent's "See all") shows Back; the drawer has no category list and shows the app
  icon (so do Settings and About); tapping an entry opens its overview (when, account, card,
  whether it repeats) with Edit; Entries' tab icon is a clock, not three lines.
- **Dark pictures share one ground (0.3.2).** Every Dark picture sits on the same charcoal with a
  soft warm glow behind the object; a render that comes out near-black is lifted onto it at export
  (the owner saw the chart card as "darker than the rest").
- **App icon.** Icon E, "fanned cards": a black, a cream and a sage card fanned, on near-black (Dark)
  or white to stone (Light). The launcher icon follows the app's palette.

## Direction C, "Postcard sky" (chosen and built 2026-11-19)

Asked for by the owner: "it doesn't really look like the app it should look like most are somewhat
bright with pictures and colors". Of three bright directions drawn on the Relay design canvas
(design 40, "Tally: bright directions + bank connect"), the owner picked **C** and asked for every
screen in that style before it is built. This replaces the Relay dark look above once built; until
then the code still wore the Relay tokens. Built: `ui/theme/Theme.kt` (Sky, Dusk, Mint, Night), `ui/kit/Sky.kt`, the bottom bar in `ui/shell/Frame.kt`.

- **Ground**: a sky gradient (#A9D8FF top, #E6E4FF middle, #FFE4D4 bottom) with soft white clouds.
- **Surfaces**: frosted white glass cards (white at 72 to 80%, background blur, 22 to 24 radius);
  solid white "postcard" cards with a soft shadow for goals and anything with a picture.
- **Ink**: #1A1A2E, #4A4A63 supporting, #626279 captions. Money in is green #0B8457.
- **The month**: a half-circle gauge, green to amber to coral as the month fills, the amount left
  in the middle.
- **Pictures**: goals and categories carry a picture (photo or illustrated postcard). Bills carry
  coloured date badges.
- **Navigation**: a frosted bottom bar (Overview, Plan, Accounts, Invest) with a dark square Add key
  in the middle.
- **Where the screens are**: design 40, pages "C · Spend", "C · Plan", "C · Money" (with the Plaid
  connect flow) and "C · Setup & settings": 34 artboards covering every route.
- **Postcard pictures** (2026-11-19): made on the owner's PC with Qwen-Image 2.1
  (`~/ai/qwen-image`), one soft pastel illustrated style that matches the ground: mountain, beach,
  growth, hills, city, road, and a plain sky behind the bank tiles. They live in
  `res/drawable-nodpi/scene_*.webp` (1280×512, cropped to fit) and are dimmed a little on Night.
  Add a scene in the same style and prompt suffix rather than mixing in stock photos.
- **Type and voice** are unchanged: Geist, Sora for the wordmark, tabular amounts, the voice in
  PRODUCT.md.
