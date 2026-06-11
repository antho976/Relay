package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import com.forge.app.data.db.entities.SuggestionOutcome

@Dao
interface SuggestionOutcomeDao {

    @Insert
    suspend fun insert(outcome: SuggestionOutcome)

    @Query("SELECT * FROM suggestion_outcome ORDER BY logged_at DESC LIMIT :limit")
    suspend fun recent(limit: Int = 500): List<SuggestionOutcome>

    @Query("DELETE FROM suggestion_outcome")
    suspend fun deleteAll()
}
