package com.forge.app.data.repo

import com.forge.app.data.db.dao.VacationDao
import com.forge.app.data.db.entities.VacationPeriod
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Holiday / vacation ranges (#135). A range pauses the training streak — its days are
 * bridged in the streak math (see [com.forge.app.domain.vacation.VacationCalendar]).
 */
@Singleton
class VacationRepository @Inject constructor(
    private val dao: VacationDao
) {
    fun observeAll(): Flow<List<VacationPeriod>> = dao.observeAll()

    /** Add a range. Start/end are "yyyy-MM-dd"; order is normalized so start ≤ end. */
    suspend fun add(startDate: String, endDate: String, label: String) {
        val (s, e) = if (startDate <= endDate) startDate to endDate else endDate to startDate
        dao.insert(VacationPeriod(startDate = s, endDate = e, label = label.trim()))
    }

    suspend fun delete(period: VacationPeriod) = dao.delete(period)
}
