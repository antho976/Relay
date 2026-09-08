package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/** A watch command outcome committed atomically with its workout mutation. */
@Entity(tableName = "wear_command")
data class WearCommand(
    @PrimaryKey @ColumnInfo(name = "command_id") val commandId: String,
    @ColumnInfo(name = "ack_json") val ackJson: String,
    @ColumnInfo(name = "completed_at") val completedAt: Long
)
