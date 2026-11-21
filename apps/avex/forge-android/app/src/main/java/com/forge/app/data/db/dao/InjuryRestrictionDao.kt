package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Query
import com.forge.app.data.db.entities.InjuryRestriction

/** Injury restrictions (Coach v3 B1). Cleared rows are kept — they explain that month's numbers. */
@Dao
interface InjuryRestrictionDao {

    @Query("SELECT * FROM injury_restriction WHERE cleared_at IS NULL ORDER BY started_at DESC")
    suspend fun active(): List<InjuryRestriction>
}
