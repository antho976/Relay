package com.forge.app.data.repo

import com.forge.app.data.db.dao.VacationDao
import com.forge.app.data.db.entities.VacationPeriod
import kotlinx.coroutines.flow.Flow
import java.time.LocalDate
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class VacationRepository @Inject constructor(private val dao: VacationDao) {

    fun observeAll(): Flow<List<VacationPeriod>> = dao.observeAll()

    suspend fun add(startDate: String, endDate: String, label: String = ""): Long =
        dao.insert(VacationPeriod(startDate = startDate, endDate = endDate, label = label))

    suspend fun delete(period: VacationPeriod) = dao.delete(period)

    /** Returns true if the given date falls within any vacation period. */
    suspend fun isOnVacation(date: LocalDate): Boolean {
        val key = date.toString()
        return dao.all().any { v -> key >= v.startDate && key <= v.endDate }
    }

    /** All vacation date keys (yyyy-MM-dd) across all periods. */
    suspend fun allVacationDateKeys(): Set<String> {
        val keys = mutableSetOf<String>()
        dao.all().forEach { v ->
            var d = LocalDate.parse(v.startDate)
            val end = LocalDate.parse(v.endDate)
            while (!d.isAfter(end)) {
                keys.add(d.toString())
                d = d.plusDays(1)
            }
        }
        return keys
    }
}
