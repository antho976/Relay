package com.forge.app.ui.overview

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.db.entities.WarmupRoutineItem
import com.forge.app.data.repo.CustomizationRepository
import com.forge.app.data.repo.ProgramCustomizationRepository
import com.forge.app.data.repo.WarmupRepository
import com.forge.app.program.MuscleGroup
import com.forge.app.program.Program
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class DayEditViewModel @Inject constructor(
    private val customizationRepo: CustomizationRepository,
    private val programCustomRepo: ProgramCustomizationRepository,
    private val warmupRepo: WarmupRepository
) : ViewModel() {

    private val _selectedDayKey = MutableStateFlow(Program.dayKeys.first())
    val selectedDayKey: StateFlow<String> = _selectedDayKey

    fun selectDay(key: String) { _selectedDayKey.value = key }

    val customName: StateFlow<String?> = combine(
        _selectedDayKey,
        customizationRepo.observeAllDayNames()
    ) { key, names ->
        names.firstOrNull { it.dayKey == key }?.customName
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    val exerciseCustomizations = _selectedDayKey
        .flatMapLatest { key -> programCustomRepo.observeForDay(key) }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    val warmupItems: StateFlow<List<WarmupRoutineItem>> = _selectedDayKey
        .flatMapLatest { key -> warmupRepo.observeForDay(key) }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun setDayName(name: String) = viewModelScope.launch {
        val key = _selectedDayKey.value
        if (name.isBlank()) customizationRepo.clearDayName(key)
        else customizationRepo.setDayName(key, name)
    }

    fun setSetsOverride(exerciseId: String, sets: Int) = viewModelScope.launch {
        programCustomRepo.setSetsOverride(_selectedDayKey.value, exerciseId, sets)
    }

    fun setRepsOverride(exerciseId: String, reps: String) = viewModelScope.launch {
        if (reps.isNotBlank()) programCustomRepo.setRepRange(_selectedDayKey.value, exerciseId, reps)
    }

    fun toggleExerciseRemoved(exerciseId: String, currentlyRemoved: Boolean) = viewModelScope.launch {
        if (currentlyRemoved) programCustomRepo.restoreExercise(_selectedDayKey.value, exerciseId)
        else programCustomRepo.removeExercise(_selectedDayKey.value, exerciseId)
    }

    fun addCustomExercise(name: String, muscle: MuscleGroup) = viewModelScope.launch {
        programCustomRepo.addCustomExercise(_selectedDayKey.value, name, muscle)
    }

    fun resetExercises() = viewModelScope.launch {
        programCustomRepo.resetDay(_selectedDayKey.value)
    }

    fun addWarmupItem(label: String) = viewModelScope.launch {
        warmupRepo.addItem(_selectedDayKey.value, label)
    }

    fun removeWarmupItem(item: WarmupRoutineItem) = viewModelScope.launch {
        warmupRepo.removeItem(item)
    }

    fun resetWarmup() = viewModelScope.launch {
        warmupRepo.resetDay(_selectedDayKey.value)
    }
}
