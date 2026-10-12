package com.tally.app.ui.invest

import androidx.compose.runtime.Immutable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.tally.app.data.Clock
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.InvestRepository
import com.tally.app.ui.common.DRAFT_KEY
import com.tally.app.ui.common.Notices
import com.tally.app.ui.nav.Args
import com.tally.app.ui.plan.PadKey
import com.tally.app.ui.plan.pressed
import com.tally.core.AmountInput
import com.tally.core.Invest
import com.tally.core.MoneyFormatter
import com.tally.core.Portfolio
import com.tally.core.Registration
import com.tally.core.RoomLine
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.util.Locale
import javax.inject.Inject

/** Everything the room editor draws. */
@Immutable
data class RoomEditState(
    val registration: Registration,
    val year: Int,
    val today: LocalDate,
    val loaded: Boolean = false,
    /** The figure as stored for [year]; null when there is none. */
    val existing: Long? = null,
    val input: AmountInput = AmountInput(),
    /** What went in over [year]'s window, as the portfolio counts it. */
    val contributed: Long = 0,
    /** The year's dollar limit for the kind, offered as a fill; null where the room is not a fixed figure. */
    val limit: Long? = null,
) {
    val deadline: LocalDate get() = roomDeadline(registration, year)
}

/**
 * The room editor: this year's room for one registration, the figure CRA My Account gives (or the
 * Notice of Assessment for an RRSP), typed on the keypad. Tally subtracts what goes in; it never
 * works the room out from a birth date.
 */
@HiltViewModel
class RoomEditViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val invest: InvestRepository,
    private val settings: SettingsRepository,
    private val notices: Notices,
    clock: Clock,
) : ViewModel() {

    private val registration: Registration =
        registrationNamed(savedStateHandle.get<String>(Args.KIND))?.takeIf { it in ROOM_KINDS } ?: Registration.TFSA
    private val today: LocalDate = clock.today()
    // An RRSP's year runs to its deadline: until then, room goes on last year's figure.
    private val year: Int = Invest.roomYear(registration, today)

    private val input = MutableStateFlow(AmountInput())
    private val ready = MutableStateFlow(false)
    private val finished = Channel<Unit>(Channel.CONFLATED)

    /** Fires once after a save or a removal; the route goes back on it. */
    val done: Flow<Unit> = finished.receiveAsFlow()

    /** One write per visit. */
    private var busy = false

    /** This year's line for the kind; null when the portfolio has none, or could not be read. */
    private val line: Flow<RoomLine?> = invest.portfolio()
        .map<Portfolio, RoomLine?> { p -> p.room.firstOrNull { it.registration == registration && it.year == year } }
        .catch { emit(null) }

    val state: StateFlow<RoomEditState> = combine(line, input, ready) { found, typed, isReady ->
        RoomEditState(
            registration = registration,
            year = year,
            today = today,
            loaded = isReady,
            existing = found?.room,
            input = typed,
            contributed = found?.contributed ?: 0L,
            limit = found?.limit,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), RoomEditState(registration, year, today))

    init {
        viewModelScope.launch {
            // What was typed before the process died wins over the stored figure.
            val kept = savedStateHandle.get<String>(DRAFT_KEY)
            val digits = MoneyFormatter(settings.current().currency, Locale.getDefault()).fractionDigits
            val stored = line.first()?.room
            input.value = when {
                kept != null -> AmountInput(kept, digits)
                stored != null -> AmountInput.of(stored, digits)
                else -> AmountInput(fractionDigits = digits)
            }
            ready.value = true
            input.collect { savedStateHandle[DRAFT_KEY] = it.text }
        }
    }

    fun press(key: PadKey) = input.update { it.pressed(key) }

    /** A quick fill: the year's limit, or the figure stored before. */
    fun fill(amount: Long) = input.update { AmountInput.of(amount, it.fractionDigits) }

    fun save() {
        if (busy) return
        val amount = input.value.minor
        if (amount <= 0L) return
        busy = true
        val label = registrationLabel(registration)
        viewModelScope.launch {
            invest.setRoom(registration, year, amount)
            notices.show("$label room set for $year")
            finished.send(Unit)
        }
    }

    /** Removes this year's figure at once and offers Undo; nothing asks first. */
    fun remove() {
        if (busy) return
        val old = state.value.existing ?: return
        busy = true
        val repo = invest
        val kind = registration
        val y = year
        viewModelScope.launch {
            // A figure of zero or less removes it (the contract's room op).
            repo.setRoom(kind, y, 0L)
            // The undo holds the repository and the figure, never this ViewModel.
            notices.showUndo(registrationLabel(kind) + " room removed") { repo.setRoom(kind, y, old) }
            finished.send(Unit)
        }
    }
}
