package com.forge.app.ui.gym.train.state

import com.forge.app.data.db.entities.Session
import com.forge.app.program.DayPlan

data class DayListUiState(
    val days: List<DayListItem> = emptyList(),
    val activeSession: Session? = null
)

data class DayListItem(
    val plan: DayPlan,
    val displayName: String,
    val lastFinishedAt: Long? = null,
    val isActive: Boolean = false,
    val isNextUp: Boolean = false,
    val exerciseCount: Int = 0
)
