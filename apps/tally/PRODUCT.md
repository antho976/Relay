# Product

## Platform

Android, for one person tracking their own money (the owner), who also runs Relay on a PC and a
Discord bot on a server.

## Purpose

Tally records spending and income and answers one question at all times: how much room is left in
this month's budget, and is the month on pace. Success is every purchase logged because it costs
nothing, and never being surprised at the end of the month.

## The second Tally (2026-10-12)

Asked for by the owner: "a new Tally app ... improved on functions and UX/UI, make it look like the
Relay mobile app ... connect it to the server which is hosted on antho's bot repo ... it should
automatically be synced with nothing for me to do while being on the same wifi or not."

- **Relay's look**, not the first Tally's Avex look: Relay's phone app's tokens, fonts (Geist,
  Geist Mono, Sora), glyphs and frame (top bar with a space switch, drawer, status bar).
- **Sync with nothing to set up**: the app connects to the owner's server by itself; the one act is
  a tap on Allow in Discord, once per phone, because a ledger must not open to whoever installs the
  APK. The first sync merges, so neither side loses anything.
- **The ledger's rules and data are the first Tally's**: the money rules (`core/`), the Room schema,
  and the sync protocol RelayV4 defines (`docs/MONEY.md`, "Sync"), so the server, the PC and both
  Tallys read the same rows.

## Spending, not balances (2026-10-13)

Asked for by the owner: "it should track the spending not the total account money so no negative
just says if you hit the limit". Tally reads spending against the month's limit; it does not
keep what the accounts hold unless an account was given a balance to start from.

- No reading is ever a negative figure. Past the limit, Tally says "Limit reached" and by how much
  ("$120 past your $3,000 limit"); a tracked balance below zero reads in words ("owes $412.20").
- Accounts are what you pay with. Overview, the editor and the status bar never show a balance;
  Accounts leads with what each one paid out this month.

## Principles

1. **Logging is free.** Add is one tap from every page and from the launcher, and finishes in three
   more.
2. **Show the reading, not just the verdict.** "Over pace" always travels with the amount and the
   days.
3. **Honest at zero.** No data, no budget, no accounts: each state is drawn, never hidden.
4. **Nothing leaves the owner's own devices.** The phone, the owner's server, the owner's PC.
5. **Reversible by default.** Deletes come with Undo; only erasing everything, replacing a ledger
   wholesale, or deleting an account with entries asks first.

## Voice

Dry, specific, grounded in the owner's own numbers. No exclamation marks, no em dashes in the app's
words, no praise the data does not support. Speaks in "you". Machine identifiers never render.
