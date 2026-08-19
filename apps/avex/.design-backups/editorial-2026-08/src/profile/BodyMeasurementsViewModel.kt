package com.forge.app.ui.profile

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.BodyMeasurementEntry
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.BodyMeasurementRepository
import com.forge.app.domain.measurement.BodyMeasurementType
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/** One measurement type with its readings oldest → newest (empty when never logged). */
data class MeasurementSeries(
    val type: BodyMeasurementType,
    val entries: List<BodyMeasurementEntry>
)

data class BodyMeasurementsUiState(
    /** All five types, in display order — a type with no readings carries an empty list. */
    val series: List<MeasurementSeries> = BodyMeasurementType.entries.map { MeasurementSeries(it, emptyList()) },
    val useCm: Boolean = false
) {
    val trackedCount: Int get() = series.count { it.entries.isNotEmpty() }
    val anyData: Boolean get() = trackedCount > 0
}

/**
 * Backs the Measurements sub-screen (GYMAP-52) and the profile-hub summary card. Groups the flat
 * measurement log into one series per type (oldest → newest for the sparklines) and folds in the
 * user's cm/in preference. Local-only via [BodyMeasurementRepository].
 */
@HiltViewModel
class BodyMeasurementsViewModel @Inject constructor(
    private val repo: BodyMeasurementRepository,
    settings: SettingsRepository
) : ViewModel() {

    val state: StateFlow<BodyMeasurementsUiState> =
        combine(repo.observeAll(), settings.useCm) { entries, useCm ->
            val byType = entries.groupBy { it.type }
            BodyMeasurementsUiState(
                series = BodyMeasurementType.entries.map { type ->
                    MeasurementSeries(
                        type = type,
                        entries = (byType[type.key] ?: emptyList()).sortedBy { it.recordedAt }
                    )
                },
                useCm = useCm
            )
        }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), BodyMeasurementsUiState())

    /** Record today's readings — one upsert per supplied (type, cm) pair. */
    fun log(values: List<Pair<BodyMeasurementType, Double>>) = viewModelScope.launch {
        values.forEach { (type, cm) -> repo.log(type, cm) }
    }

    fun delete(id: Long) = viewModelScope.launch { repo.delete(id) }
}
