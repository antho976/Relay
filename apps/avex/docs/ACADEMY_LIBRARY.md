# Avex Academy — the Library (authoring guide)

> The Academy's open half, added 2026-08-15. Sibling to `ACADEMY_LESSONS.md`, which owns the
> earned half. That doc is the curriculum; this one is the shelf.
>
> Machinery: `domain/academy/Article.kt` · `ArticleRegistry` · `LibraryRepository` ·
> `ui/academy/LibraryPane.kt` + `ArticleScreen.kt` · `article_event` (schema v36).
> Design record: `.claude/design/MAP.md`, Academy.

## What the Library is, and what it is not

A lesson is **earned**. It attaches to a coach moment and opens the first time that moment fires,
which is what makes it honest: it explains something that just happened to you.

An article is **not**. It is readable from install by anyone, in any order, forever. It exists
because the subject is worth knowing whether or not the coach ever touches it.

Keeping those two contracts visibly different is the whole reason they can share a tab without
reading as duplicates. If an article would only make sense after the coach did something, it is a
lesson and belongs in the other doc.

## Rules that shape this list

- **Sourced, or it does not ship.** Every article carries at least one `Source`, and
  `ArticleRegistryTest.everyArticleIsSourced` fails the build otherwise. This is the only mechanical
  guard on the Library being research condensed rather than opinion typed confidently.
- **Verify every citation against the actual paper before shipping it.** Authors, title, journal
  and year, all four. A plausible-looking citation nobody opened is how invented papers get into
  products, and the Library's entire premise dies the first time one is found. Track status in the
  table below.
- **Sources render as plain text, at the end, never as links.** The app holds no INTERNET
  permission and will not gain one; a tappable citation would be an affordance that cannot run.
  There are no inline `[1]` markers either, because they turn prose into a paper, and the point is
  that this reads.
- **Prose, not bullets.** An article earns its length by explaining a mechanism and then saying what
  to do differently. `Bullets` stays available for the rare genuinely enumerable thing; more than
  one bullet block in an article means the paragraphs were never written.
- **1 to 10 minutes, ceiling about 30.** Past thirty it is a book, not a lesson. Read time is
  **derived from word count** (200 wpm, `Article.readMinutes`), never authored, so it cannot drift
  when a paragraph is edited.
- **Say where the evidence runs out.** "The mechanisms are not fully settled" is a sentence the
  Library is allowed to write and should. Overclaiming is the failure mode this shelf exists to
  avoid.
- **Voice is DESIGN §11.** Dry, specific, imperative and "you". No exclamation marks, no em dashes,
  no hype, no praise ungrounded in data. `DesignDoctrineTest` scans `domain/` for the mechanical
  half of that, so a banned character fails the build.
- **No XP, no streaks, no percentage complete.** The plan's ban on gamifying the Academy applies
  here at least as hard as it does to lessons.

## Levels

A **label beside the read time**, never a filter and never a gate. Difficulty is a question a reader
can only answer after opening something, so filtering by it would hide articles behind a judgement
they have not made yet. The filter axis is topic.

| Level | Assumes |
|---|---|
| **Basics** | nothing. The idea itself, and why it matters. |
| **Applied** | you train. The numbers, and what to do differently on Monday. |
| **Deep** | you have read the Applied one. Mechanism, disagreement in the literature, and where the evidence runs out. |

## Topics

Eight shelves in `ArticleTopic`, **but the index renders only the ones that hold an article**
(`ArticleRegistry.topicsWithContent`). A shelf appears the day its first article does, so the
Library grows visibly instead of opening as six empty rooms.

The split follows how readers actually look for training material rather than how a coach thinks
about it. "Training" as one shelf was the first draft and it was wrong: hypertrophy, programming and
technique are three unrelated questions that happen to share a gym. Cross-checked against how
Stronger by Science files its archive, which is the nearest thing to a reference standard here.

| Topic | Holds |
|---|---|
| Hypertrophy | growth: volume, proximity to failure, rep ranges, range of motion |
| Strength | force production, intensity, specificity, peaking |
| Programming | blocks, periodisation, frequency, deloads, autoregulation |
| Technique | the lifts themselves, and what actually varies between good ones |
| Nutrition | protein, energy balance, supplements, timing |
| Recovery | sleep, stress, soreness, pain versus injury |
| Conditioning | cardio in service of lifting (pairs with the Engine lesson track) |
| Mindset | adherence, expectations, what progress actually looks like |

## Linking from the coach

Articles are namespaced `library.*`; lessons are not. `AcademyLink.resolve` reads that namespace, so
**any existing nullable `lessonId` slot can carry an article id** with no schema change and no
call-site churn — `Recommendation`, `TodayDirective`, `CoachSignal`, `ConditioningPlanner`,
`GoalPortfolio`. `ArticleRegistryTest` pins the two id spaces disjoint so this can never become
ambiguous.

Use it when the coach's reason is genuinely about the subject rather than about the decision. A
stall the coach declined to escalate is a lesson; "why ten sets and not twenty" is an article.

## Shipped

| id | Topic | Level | Sources verified |
|---|---|---|---|
| `library.proximity_to_failure` | Hypertrophy | Applied | **not yet** |
| `library.how_much_volume` | Hypertrophy | Applied | **not yet** |
| `library.sleep_and_training` | Recovery | Basics | **not yet** |
| `library.protein_intake` | Nutrition | Applied | **not yet** |

**All four seed articles were written with citations from memory and are pending a verification
pass.** The claims in the prose are mainstream and well supported; the risk is in the citation
metadata (wrong journal, wrong year, wrong author order), which is exactly the kind of error that
looks fine and is not. Verify before any release that ships the Library.

## Backlog

Nothing committed. Candidates that follow naturally from what is already here, roughly in order of
how often the coach would want to link to them:

- Rep ranges and load: why 5 and 30 both grow muscle, and why the middle is convenient
- Frequency: what changes when the same weekly volume is split differently
- Range of motion and long-muscle-length training, including how soft the evidence still is
- What a deload is for, and how to tell you needed one
- Progressive overload past the beginner phase, where the load stops going up every week
- Soreness is not the signal (pairs with `fundamentals.soreness_vs_injury`)
- Energy balance: why the scale moves before anything real does
- Creatine, the one supplement worth a page
