# Product

<!-- impeccable:product-schema 1 -->

> **Provenance.** The owner asked for this app to be built without an interview ("do not ask me
> about what to do with the UI"), so every fact below marked *(inferred)* was derived from the
> original brief and from the sibling app Avex, not confirmed in a question round. Facts marked
> *(brief)* are the owner's own words. Revisit the inferred ones after the first real use.

## Platform

android

## Stack

delegated, with two pins from the brief: Kotlin for all code and SQLite for storage *(brief)*.
Chosen: Jetpack Compose + Material 3, Room over SQLite, Hilt, Kotlin coroutines and Flow,
DataStore for preferences, WorkManager for posting recurring entries. Same toolchain generation as
Avex so one person can maintain both *(inferred)*.

## Users

One person tracking their own money on their own phone *(inferred)*. Two scenes, both inferred
from the brief "tracking my spending, budget etc":

- **The two-second log.** Right after paying for something: open, type the amount, pick a
  category, done. This is the most frequent act in the product and must be the cheapest one.
- **The check-in.** A few times a week, usually evening, at home: "how much room do I have left
  this month, and where did it go?" Read, maybe adjust a budget, leave.

## Product Purpose

Tally *(the owner's name for it, 2026-10-04; the working name was Margin)* records spending and income into local SQLite and answers one
question at all times: how much room is left in this month's budget, and is the month on pace.
Success is the owner logging every purchase because it costs nothing, and never being surprised at
the end of the month.

## Positioning

**The month is read as a pace, not a total.** Every budget is measured against where an even
spend would put it today, so "you have spent 62%" becomes "you are 4 days ahead of your money".
The reading travels with the number it came from. Fully offline: no account, no bank sync, no
servers, no internet permission *(inferred from Avex's offline promise)*.

## Operating Context

- A budget period is a calendar month by default, with a configurable start day for people paid
  on a fixed date *(inferred)*.
- Entries are expenses, income, or transfers between the owner's own accounts.
- Accounts are cash, bank, credit card or savings, each with a starting balance.
- Recurring entries (rent, subscriptions, salary) post themselves on their due date.
- Savings goals hold a target and contributions.
- Data leaves the phone only when the owner exports it (CSV, JSON backup) *(inferred)*.

## Capabilities and Constraints

- All data is local (Room). Reads are instant; there is no network latency to design loading
  states around.
- Money is stored as integer minor units (cents), never floating point.
- Currency is chosen by the owner, defaulting to the device locale *(inferred; the owner is
  probably in Québec, so CAD and French-Canadian number formats must render correctly)*.
- Undecided: bank import, multi-currency accounts, shared budgets, cloud sync. None are built.

## Brand Commitments

- Visual reference: Avex, "something like that but adapted to spending" *(brief)*. Inherited:
  warm near-black ground, three type voices (serif figures, sans rows, mono labels), one
  user-chosen accent.
- **Adapted, not copied (binding, owner 2026-10-04):** "I want it to feel professional with icons,
  backgrounds, more details since it's not for the gym; gym apps are more discreet compared to what
  I want." So Tally is richer than Avex: sections sit on raised panels, every section and stat is
  led by a glyph, the hero reading sits in a lit panel, and screens carry more readings and charts
  than Avex's open page would.
- Voice inherited from Avex *(inferred)*: dry, specific, grounded in the owner's own numbers.
  No exclamation marks, no em dashes, no hype, no praise the data does not support. Speaks in
  "you", never "I".
- Machine identifiers never render: dates become human dates, enums become words.

## Evidence on Hand

- No real user data, testimonials, or metrics exist. The app ships a "Load sample data" action
  for testing that is labeled synthetic everywhere it appears.
- No pricing, no store listing, no benchmarks. None may be invented.

## Product Principles

1. **Logging is free.** The add flow is reachable in one tap from every main screen and finishes
   in three more.
2. **Show the reading, not just the verdict.** "Over pace" always travels with the amount and
   the days.
3. **Honest at zero.** No data, no budget, no accounts: each state is drawn, never hidden.
4. **Nothing leaves the phone unasked.**
5. **Reversible by default.** Deletes come with Undo, not a confirm dialog; only erasing
   everything confirms.

## Accessibility & Inclusion

Inherited from Avex *(inferred)*: 200% font scale without clipping, TalkBack with value-reading
descriptions on every drawn chart, 48dp touch targets, RTL-correct layout, text contrast 4.5:1 or
better on the near-black ground, and a monochrome mode for people who turn the accent off.
