package com.forge.app.domain.cardio

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.DirectionsBike
import androidx.compose.material.icons.automirrored.filled.DirectionsRun
import androidx.compose.material.icons.automirrored.filled.DirectionsWalk
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.FitnessCenter
import androidx.compose.material.icons.filled.Hiking
import androidx.compose.material.icons.filled.Hotel
import androidx.compose.material.icons.filled.MoreHoriz
import androidx.compose.material.icons.filled.Pool
import androidx.compose.material.icons.filled.Rowing
import androidx.compose.material.icons.filled.SelfImprovement
import androidx.compose.ui.graphics.vector.ImageVector

/**
 * The fixed set of cardio activity types. The DB stores [code] as a String so the
 * schema stays open-ended, but the UI only knows these values. [REST] is special:
 * it represents a recovery day, not a workout, and is excluded from "minutes this
 * week" totals (see CardioDao.observeMinutesSince's `excludeType` param).
 */
enum class CardioType(
    val code: String,
    val displayName: String,
    val icon: ImageVector
) {
    RUN("run", "Run", Icons.AutoMirrored.Filled.DirectionsRun),
    WALK("walk", "Walk", Icons.AutoMirrored.Filled.DirectionsWalk),
    // Treadmill = indoor running (the meditation glyph it used to have now goes to Yoga, where it fits).
    TREADMILL("treadmill", "Treadmill", Icons.AutoMirrored.Filled.DirectionsRun),
    CYCLE("cycle", "Cycle", Icons.AutoMirrored.Filled.DirectionsBike),
    SWIM("swim", "Swim", Icons.Filled.Pool),
    ROW("row", "Row", Icons.Filled.Rowing),
    HIKE("hike", "Hike", Icons.Filled.Hiking),
    ELLIPTICAL("elliptical", "Elliptical", Icons.Filled.FitnessCenter),
    HIIT("hiit", "HIIT", Icons.Filled.Bolt),
    YOGA("yoga", "Yoga", Icons.Filled.SelfImprovement),
    REST("rest", "Rest Day", Icons.Filled.Hotel),
    OTHER("other", "Other", Icons.Filled.MoreHoriz);

    val isRest: Boolean get() = this == REST

    companion object {
        fun fromCode(code: String): CardioType =
            entries.firstOrNull { it.code == code } ?: OTHER
    }
}
