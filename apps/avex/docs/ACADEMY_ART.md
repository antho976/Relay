# Academy cover art — what's missing

The Academy ships **35 pieces** (31 lessons + 4 articles). **6 have cover art**, all of them
Fundamentals. The other **29 render as index entries** — a title and its deck, with no plate above
them. The gallery is built so that degrades gracefully (a chapter with no art reads as a contents
page rather than as a wall of empty frames), but the rhythm the page is designed around — a
full-width lead plate, then two-up posters — only exists where there are pictures.

This file is the manifest for that gap. One image per piece, plus the house rules the six
existing covers already follow.

---

## House style (from the six that exist)

Black-and-white chiaroscuro still life. One object, one hard light source raking across it from
the side, everything else falling into near-black. Aged, industrial, physical: cast iron, brass,
chalk, worn leather, ruled paper. No people, no faces, no hands, no text, no logos, no gradients,
no digital-render sheen.

The six already in the app, for reference:

| Piece | Subject |
|---|---|
| `fundamentals.what_a_program_is` | open ruled ledger, pencil in the gutter |
| `fundamentals.sets_reps_rpe` | chalk tally marks on a dark slab |
| `fundamentals.form_vs_load` | barbell sleeve in a rack upright |
| `fundamentals.rest_and_recovery` | flat bench, towel draped over the end |
| `fundamentals.how_the_coach_works` | brass balance scale, level |
| `fundamentals.what_readiness_means` | pressure gauge, needle mid-dial |

## File spec

- **One master per piece, 3:4 portrait, 1200×1600**, exported `.webp` (quality ~80, the existing
  set lands at 30–90 KB).
- Drop into `forge-android/app/src/main/res/drawable-nodpi/`, named
  `cover_<id-after-the-dot>.webp` — e.g. `programming.four_phases` → `cover_four_phases.webp`.
- **Composition:** subject centred, filling most of the frame. Nothing is printed over the picture
  any more, so there are no dead zones to protect: the gallery sets every title and meta line
  UNDER the plate, on the page. The one constraint left is the crop — a full-width lead tile takes
  a 3:2 band from the middle of the 3:4 master, so keep the subject out of the top and bottom
  eighths and it survives both shapes.
- Colour is forced to greyscale at render time, so a colour original is safe, but shoot/generate
  for tone: the picture has to survive at **165 dp across** on a phone.
- One master serves both tile shapes and the reader's own hero (which crops 4:3 and dissolves into
  the page). Nothing needs a landscape version.

## Prompt stem

Prepend to every subject line below:

> Black-and-white fine-art still life photograph, single hard raking light from the left, deep
> black shadows, high contrast, aged industrial texture, shallow depth of field, no people, no
> text, no logos, single subject centred in a 3:4 vertical frame with room around it, edges
> falling into black —

---

## Fundamentals — 4 missing

| id | Title | Subject |
|---|---|---|
| `fundamentals.progressive_overload` | Progressive overload | a row of small fractional plates stepping up in size along a dark bench |
| `fundamentals.soreness_vs_injury` | Soreness vs injury | a roll of elastic bandage, half unwound, on scarred dark wood |
| `fundamentals.warmups` | Why warm-ups are in your session | an empty barbell resting on a rack's low pins, chalk dust hanging in the light beam |
| `fundamentals.log_honestly` | Log honestly | close crop on a fountain-pen nib over a ruled logbook line, one entry crossed out |

## How the coach works — 6 missing

| id | Title | Subject |
|---|---|---|
| `coach.readiness_built_from` | What your score is built from | six brass calibration weights of graded size on slate |
| `coach.why_goals_fight` | Why some goals fight each other | two thick ropes pulling opposite ways off a single iron cleat |
| `coach.strength_on_a_cut` | Holding strength while cutting | a balance holding level with a heavy plate on one pan and an almost-empty scoop on the other |
| `coach.what_a_project_is` | What a project is | a card-index drawer, one typed card raised above the rest |
| `coach.trust_tiers` | What each trust tier means | five iron keys of increasing size hanging in a row on hooks |
| `coach.taking_decisions_back` | How to take any decision back | a heavy industrial breaker lever caught mid-throw |

## Programming — 8 missing

| id | Title | Subject |
|---|---|---|
| `programming.what_a_block_is` | What a training block is | a solid block of milled steel stock on a machinist's bench |
| `programming.four_phases` | Accumulate, intensify, peak, deload | four steel gauge blocks in a stepped row, tallest third |
| `programming.deloads_are_earned` | Deloads are earned | a heavy coil spring at rest, fully uncompressed, on dark steel |
| `programming.reading_your_block_card` | Reading your block card | a punched card lying under a desk lamp, rows of holes catching the light |
| `programming.your_volume_landmarks` | MEV and MRV | a graduated glass cylinder, fluid sitting between two etched marks |
| `programming.your_recovery_curve` | Your recovery curve | an hourglass mid-flow, sand column lit from the side |
| `programming.sweet_spot_reps` | Your sweet-spot rep ranges | a dial caliper closed on a small round bar, dial face catching light |
| `programming.imbalances` | Imbalances | a two-pan balance tipped hard to one side, close crop on the tilt |

## Signals — 1 missing

| id | Title | Subject |
|---|---|---|
| `signals.stress_hrv` | What HRV tells you | a paper trace curling off a seismograph drum, inked peaks legible |

## Conditioning — 6 missing

| id | Title | Subject |
|---|---|---|
| `engine.why_aerobic_base` | Why lifters need an aerobic base | an old leather blacksmith's bellows, half open |
| `engine.what_zone2_is` | What zone 2 actually is | a wooden metronome, pendulum arm caught off-centre |
| `engine.interference` | Interference | two heavy iron gears meshed, teeth slightly out of phase |
| `engine.reading_hr` | Reading heart rate | a mechanical stopwatch, sweep hand mid-dial, case worn |
| `engine.intervals` | Intervals | a ring-side round bell and striker, dark rope out of focus behind |
| `engine.base_without_a_lab` | How your base is measured | a brass sextant on a chart table, arc catching the light |

## Library articles — 4 missing

| id | Title | Subject |
|---|---|---|
| `library.protein_intake` | How much protein you need | a scoop of powder on the pan of a kitchen balance, dust in the beam |
| `library.proximity_to_failure` | How close to failure | a thick rope under tension, one strand frayed and lifting |
| `library.how_much_volume` | How much volume | ten identical plates leaned in a row against a dark wall |
| `library.sleep_and_training` | What sleep does for training | a wound alarm clock on a bedside table, hands at 3:10, single hard sidelight |

---

**Total: 29 images.** Fundamentals' four first if you want to see the effect before committing to
the rest — that section is already half-illustrated, so it shows the finished rhythm soonest.
