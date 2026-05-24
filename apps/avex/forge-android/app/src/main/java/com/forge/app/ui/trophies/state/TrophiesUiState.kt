package com.forge.app.ui.trophies.state

import com.forge.app.program.Trophy
import com.forge.app.program.TrophyCategory

enum class TrophyFilter(val label: String) {
    ALL("All"),
    UNLOCKED("Unlocked"),
    IN_PROGRESS("In Progress"),
    LOCKED("Locked")
}

/**
 * Catalog screen state. Trophies are pre-grouped by [TrophyCategory] in the order
 * they appear in [com.forge.app.program.Trophies.all] — UI just renders the sections
 * top-to-bottom via [filteredSections].
 */
data class NearMissEntry(
    val trophyName: String,
    val progress: Int,
    val target: Int,
    val recordedAt: Long
)

data class TrophiesUiState(
    val isLoading: Boolean = true,
    val unlockedCount: Int = 0,
    val totalCount: Int = 0,
    val sections: List<TrophySection> = emptyList(),
    val selectedFilter: TrophyFilter = TrophyFilter.ALL,
    /** "X away from [Trophy Name]" for the locked trophy with the highest progress (#55). */
    val closestTrophyNudge: String? = null,
    /** Recent near-miss trophy events (#136). */
    val nearMisses: List<NearMissEntry> = emptyList(),
    /** Sum of point values for all unlocked trophies (#150). */
    val cumulativeScore: Int = 0,
    /** Total possible score if all trophies are unlocked (#150). */
    val maxScore: Int = 0
) {
    val filteredSections: List<TrophySection>
        get() {
            if (selectedFilter == TrophyFilter.ALL) return sections
            return sections.mapNotNull { section ->
                val filtered = section.displays.filter { display ->
                    when (selectedFilter) {
                        TrophyFilter.ALL -> true
                        TrophyFilter.UNLOCKED -> display.isUnlocked
                        TrophyFilter.IN_PROGRESS -> !display.isUnlocked && (display.progressFraction ?: 0f) > 0f
                        TrophyFilter.LOCKED -> !display.isUnlocked && (display.progressFraction ?: 0f) == 0f
                    }
                }
                if (filtered.isEmpty()) null
                else section.copy(displays = filtered)
            }
        }
}

data class TrophySection(
    val category: TrophyCategory,
    val displays: List<TrophyDisplay>
)

/**
 * One trophy as the grid renders it. [unlockedAt] is non-null iff the user has earned
 * it. [progressHint] and [progressFraction] are non-null iff the trophy is locked and
 * the rule supports a numeric reading.
 */
data class TrophyDisplay(
    val trophy: Trophy,
    val unlockedAt: Long?,
    val progressHint: String?,
    /** 0f–1f progress toward unlock. Null for unlocked trophies. */
    val progressFraction: Float? = null
) {
    val isUnlocked: Boolean get() = unlockedAt != null
}
