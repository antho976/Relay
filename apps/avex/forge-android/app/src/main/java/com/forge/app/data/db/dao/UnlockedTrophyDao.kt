package com.forge.app.data.db.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.forge.app.data.db.entities.UnlockedTrophy
import kotlinx.coroutines.flow.Flow

@Dao
interface UnlockedTrophyDao {

    /** IGNORE so re-firing the same trophy unlock is a no-op rather than overwriting `unlockedAt`. */
    @Insert(onConflict = OnConflictStrategy.IGNORE)
    suspend fun unlock(trophy: UnlockedTrophy)

    @Query("SELECT * FROM unlocked_trophy ORDER BY unlocked_at DESC")
    fun observeAll(): Flow<List<UnlockedTrophy>>

    @Query("SELECT trophy_id FROM unlocked_trophy")
    fun observeUnlockedIds(): Flow<List<String>>

    @Query("SELECT trophy_id FROM unlocked_trophy")
    suspend fun unlockedIds(): List<String>
}
