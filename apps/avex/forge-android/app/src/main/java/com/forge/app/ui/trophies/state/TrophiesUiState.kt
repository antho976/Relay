package com.forge.app.ui.trophies.state

import com.forge.app.program.Trophy
import com.forge.app.program.TrophyCategory

/**
 * Catalog screen state. Trophies are pre-grouped by [TrophyCategory] in the order
 * they appear in [com.forge.app.program.Trophies.all] — UI just renders the sections
 * top-to-bottom.
 */
data class TrophiesUiState(
    val isLoading: Boolean = true,
    val unlockedCount: Int = 0,
    val totalCount: Int = 0,
    val sections: List<TrophySection> = emptyList()
)

data class TrophySection(
    val category: TrophyCategory,
    val displays: List<TrophyDisplay>
)

/**
 * One trophy as the grid renders it. [unlockedAt] is non-null iff the user has earned
 * it. [progressHint] is non-null iff the trophy is still locked and the rule supports
 * a numeric reading (currently: all of them).
 */
data class TrophyDisplay(
    val trophy: Trophy,
    val unlockedAt: Long?,
    val progressHint: String?
) {
    val isUnlocked: Boolean get() = unlockedAt != null
}
