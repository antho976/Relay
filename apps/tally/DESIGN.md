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
