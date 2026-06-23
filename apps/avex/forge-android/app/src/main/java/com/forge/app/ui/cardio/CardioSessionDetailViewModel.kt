package com.forge.app.ui.cardio

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.repo.BodyweightRepository
import com.forge.app.data.repo.CardioRepository
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import com.forge.app.ui.nav.Routes
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/** State for the routed, single-session cardio stats screen (reached from the History page). */
data class CardioSessionDetailState(
    val loaded: Boolean = false,
    val entry: CardioEntry? = null,
    val bodyweightLb: Double? = null,
    val editing: Boolean = false,
    val deleted: Boolean = false
)

/**
 * Backs the routed [CardioSessionDetailScreen]. Loads a single [CardioEntry] by id (reactively, so an
 * edit refreshes the view), and offers the same edit/delete actions the in-screen cardio overlay has —
 * so opening a cardio session from History behaves identically to opening it from the cardio tab.
 */
@HiltViewModel
class CardioSessionDetailViewModel @Inject constructor(
    private val cardioRepo: CardioRepository,
    bodyweightRepo: BodyweightRepository,
    savedStateHandle: SavedStateHandle
) : ViewModel() {

    private val cardioId: Long = savedStateHandle.get<Long>(Routes.ARG_CARDIO_ID) ?: -1L

    private val editing = MutableStateFlow(false)
    private val deleted = MutableStateFlow(false)

    val state: StateFlow<CardioSessionDetailState> = combine(
        cardioRepo.observeAll().map { list -> list.firstOrNull { it.id == cardioId } },
        bodyweightRepo.observeRecent(1).map { it.firstOrNull()?.weightLb },
        editing,
        deleted
    ) { entry, bodyweightLb, isEditing, isDeleted ->
        CardioSessionDetailState(loaded = true, entry = entry, bodyweightLb = bodyweightLb, editing = isEditing, deleted = isDeleted)
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), CardioSessionDetailState())

    fun openEdit() { editing.value = true }
    fun closeEdit() { editing.value = false }

    fun delete() {
        val entry = state.value.entry ?: return
        viewModelScope.launch {
            cardioRepo.delete(entry)
            deleted.value = true
        }
    }

    fun save(
        type: CardioType,
        durationMin: Int,
        distanceKm: Double?,
        effort: CardioEffort?,
        restReason: CardioRestReason?,
        note: String?,
        dateMs: Long,
        intervalCount: Int?,
        hrZone: String?
    ) {
        val current = state.value.entry ?: return
        viewModelScope.launch {
            cardioRepo.update(
                CardioEntry(
                    id = current.id,
                    date = dateMs,
                    type = type.code,
                    durationMin = durationMin.coerceAtLeast(0),
                    distanceKm = if (type.isRest) null else distanceKm,
                    effort = if (type.isRest) null else effort?.code,
                    restReason = if (type.isRest) restReason?.code else null,
                    note = note?.takeIf { it.isNotBlank() },
                    intervalCount = if (type == CardioType.HIIT) intervalCount?.takeIf { it > 0 } else null,
                    hrZone = if (type.isRest) null else hrZone
                )
            )
            editing.value = false
        }
    }
}
