package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import com.forge.app.data.db.entities.TrophyNearMiss
import kotlinx.coroutines.flow.Flow

@Dao
interface TrophyNearMissDao {

    /**
     * Appends a row. This is NOT an upsert: the entity's only key is the autogenerate `id` and there
     * is no unique index on `trophy_id`, so an `onConflict` strategy here would never fire. Callers
     * must clear the trophy's earlier rows themselves — see [deleteForTrophiesBefore] and
     * `TrophyRepository.recordNearMisses`. Left un-pruned, every evaluation pass appended one
     * near-duplicate per trophy in the 80–99 % band, so the [observeRecent] window stopped meaning
     * "the closest recent misses" after about a week.
     */
    @Insert
    suspend fun insert(entry: TrophyNearMiss): Long

    @Query("SELECT * FROM trophy_near_miss ORDER BY recorded_at DESC LIMIT 50")
    fun observeRecent(): Flow<List<TrophyNearMiss>>

    /** Drop near-miss rows for the given trophies — used to prune ones that have since unlocked,
     *  so an already-earned trophy can't keep showing up as a near-miss / "Up next" nudge. */
    @Query("DELETE FROM trophy_near_miss WHERE trophy_id IN (:trophyIds)")
    suspend fun deleteForTrophies(trophyIds: Collection<String>)

    /** Drop this pass's superseded rows: everything recorded for [trophyIds] before [beforeMs].
     *  Pruning AFTER the new rows are inserted (rather than deleting first) means [observeRecent]
     *  never emits a momentarily empty list to the Trophies screen. */
    @Query("DELETE FROM trophy_near_miss WHERE trophy_id IN (:trophyIds) AND recorded_at < :beforeMs")
    suspend fun deleteForTrophiesBefore(trophyIds: Collection<String>, beforeMs: Long)

    @Query("DELETE FROM trophy_near_miss")
    suspend fun deleteAll()
}
