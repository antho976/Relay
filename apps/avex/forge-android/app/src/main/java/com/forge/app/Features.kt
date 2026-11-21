package com.forge.app

/**
 * Build-time feature flags. Flip a constant and rebuild to toggle — no settings UI, nothing
 * persisted. Use this for whole subsystems we want parked but re-wireable, not per-user options.
 */
object Features {
    /**
     * Master switch for the gamification layer — ranks, XP, tiers, the offline "standing" estimate,
     * and trophies. When OFF, all of that UI is simply not emitted (the underlying engines/data are
     * untouched), so flipping this back to `true` restores every surface: the Profile rank track +
     * XP ledger cell + Standing + trophy case, the Overview trophies tile + "next trophy" nudge, and
     * the rank-up celebration. The Trophies route stays registered but unreferenced while off.
     */
    const val SHOW_GAMIFICATION = false
}
