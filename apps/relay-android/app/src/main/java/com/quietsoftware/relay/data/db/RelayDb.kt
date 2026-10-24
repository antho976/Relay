package com.quietsoftware.relay.data.db

import androidx.room.Dao
import androidx.room.Database
import androidx.room.Entity
import androidx.room.Index
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.PrimaryKey
import androidx.room.Query
import androidx.room.RoomDatabase
import androidx.room.Transaction
import androidx.room.Upsert
import kotlinx.coroutines.flow.Flow

/**
 * The phone's copy of the PC's data, in SQLite like the PC's own store. One table holds every
 * entity as the JSON the bus returned (so a newer engine's fields survive without a migration),
 * one holds cached query answers, one the outbox. Schema in `app/schemas`.
 */
@Database(entities = [EntityRow::class, QueryRow::class, OutboxRow::class], version = 1, exportSchema = true)
abstract class RelayDb : RoomDatabase() {
    abstract fun entities(): EntityDao
    abstract fun queries(): QueryDao
    abstract fun outbox(): OutboxDao
}

/** One entity: the row shown ([json]) and the PC's own version under it ([base]). */
@Entity(
    tableName = "entity",
    primaryKeys = ["kind", "id"],
    indices = [Index("kind", "project_id"), Index("kind", "parent")],
)
data class EntityRow(
    val kind: String,
    val id: String,
    @androidx.room.ColumnInfo(name = "project_id") val projectId: Long?,
    val parent: String?,
    val json: String,
    val base: String?,
    val pending: Boolean,
)

@Entity(tableName = "query")
data class QueryRow(
    @PrimaryKey val key: String,
    val json: String,
    val at: Long,
)

@Entity(tableName = "outbox", indices = [Index("seq")])
data class OutboxRow(
    @PrimaryKey val id: String,
    val seq: Long,
    val op: String,
    val payload: String,
    val label: String,
    @androidx.room.ColumnInfo(name = "created_at") val createdAt: Long,
    val state: String,
    val error: String?,
    val result: String?,
    val attempts: Int,
)

@Dao
interface EntityDao {
    @Upsert
    suspend fun upsert(rows: List<EntityRow>)

    @Query("DELETE FROM entity WHERE kind = :kind AND id IN (:ids)")
    suspend fun remove(kind: String, ids: List<String>)

    @Query("DELETE FROM entity WHERE kind = :kind AND (:projectId IS NULL OR project_id = :projectId) AND (:parent IS NULL OR parent = :parent)")
    suspend fun clearScope(kind: String, projectId: Long?, parent: String?)

    @Transaction
    suspend fun replace(kind: String, projectId: Long?, parent: String?, rows: List<EntityRow>) {
        clearScope(kind, projectId, parent)
        upsert(rows)
    }

    @Query("SELECT * FROM entity WHERE kind = :kind AND id = :id")
    suspend fun get(kind: String, id: String): EntityRow?

    @Query("SELECT * FROM entity WHERE kind = :kind AND (:projectId IS NULL OR project_id = :projectId) AND (:parent IS NULL OR parent = :parent)")
    suspend fun all(kind: String, projectId: Long?, parent: String?): List<EntityRow>

    @Query("SELECT * FROM entity WHERE kind = :kind")
    fun observe(kind: String): Flow<List<EntityRow>>

    @Query("SELECT * FROM entity WHERE kind = :kind AND project_id = :projectId")
    fun observeProject(kind: String, projectId: Long): Flow<List<EntityRow>>

    @Query("SELECT * FROM entity WHERE kind = :kind AND parent = :parent")
    fun observeParent(kind: String, parent: String): Flow<List<EntityRow>>

    @Query("SELECT * FROM entity WHERE kind = :kind AND id = :id")
    fun observeOne(kind: String, id: String): Flow<EntityRow?>

    @Query("SELECT COUNT(*) FROM entity WHERE pending = 1")
    fun observePending(): Flow<Int>

    @Query("DELETE FROM entity")
    suspend fun clear()
}

@Dao
interface QueryDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun put(row: QueryRow)

    @Query("SELECT * FROM `query` WHERE `key` = :key")
    suspend fun get(key: String): QueryRow?

    @Query("SELECT * FROM `query` WHERE `key` = :key")
    fun observe(key: String): Flow<QueryRow?>

    @Query("DELETE FROM `query` WHERE `key` = :key")
    suspend fun delete(key: String)

    @Query("DELETE FROM `query`")
    suspend fun clear()
}

@Dao
interface OutboxDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun put(row: OutboxRow)

    @Query("SELECT * FROM outbox WHERE state != 'Done' ORDER BY seq")
    suspend fun open(): List<OutboxRow>

    @Query("SELECT * FROM outbox WHERE state != 'Done' ORDER BY seq")
    fun observeOpen(): Flow<List<OutboxRow>>

    @Query("SELECT * FROM outbox WHERE id = :id")
    suspend fun get(id: String): OutboxRow?

    @Query("SELECT * FROM outbox WHERE id = :id")
    fun observe(id: String): Flow<OutboxRow?>

    @Query("DELETE FROM outbox WHERE id = :id")
    suspend fun delete(id: String)

    @Query("SELECT COALESCE(MAX(seq), 0) + 1 FROM outbox")
    suspend fun nextSeq(): Long

    /** The contract of [OutboxStore.prune]: a finished entry an open one still names (`{"$ref": "<id>"}`) stays. */
    @Query(
        "DELETE FROM outbox WHERE state = 'Done' AND created_at < :before " +
            "AND NOT EXISTS (SELECT 1 FROM outbox AS o WHERE o.state != 'Done' AND instr(o.payload, '\"' || outbox.id || '\"') > 0)",
    )
    suspend fun prune(before: Long)

    @Query("DELETE FROM outbox")
    suspend fun clear()
}
