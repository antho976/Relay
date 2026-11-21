package com.forge.app.data.db.entities

/**
 * Origin tag for an overlay row (program_customization / exercise_customization).
 *
 * Both the user (day editor, program editor, train-screen swap) and the auto-coach write through
 * the SAME overlay tables, and a slot has exactly one row per (day, exercise). Before this tag the
 * coach faked "is this slot user-owned?" with an all-time `coachTouched` subtraction, which both
 * stripped lock protection from genuine user edits on any coach-touched slot and let inert coach
 * residue masquerade as a user customization (auto-coach seam audit, 2026-06-12, findings 5 + 6).
 *
 * With the tag the coach-lock scan keys off real origin, and undo can compare-and-restore instead
 * of clobbering a user's later edit. Regeneration clears coach-origin swap rows (the bias carries
 * the learning forward); user-origin rows follow the existing reconcile rules.
 */
object OverlaySource {
    const val USER = "user"
    const val COACH = "coach"
}
