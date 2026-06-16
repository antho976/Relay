package com.forge.app.domain.cardio

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.DirectionsBike
import androidx.compose.material.icons.filled.DirectionsRun
import androidx.compose.material.icons.filled.DirectionsWalk
import androidx.compose.material.icons.filled.FitnessCenter
import androidx.compose.material.icons.filled.Hiking
import androidx.compose.material.icons.filled.Hotel
import androidx.compose.material.icons.filled.MoreHoriz
import androidx.compose.material.icons.filled.Pool
import androidx.compose.material.icons.filled.Rowing
import androidx.compose.material.icons.filled.SelfImprovement
import androidx.compose.material.icons.filled.Spa
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
    RUN("run", "Run", Icons.Filled.DirectionsRun),
    WALK("walk", "Walk", Icons.Filled.DirectionsWalk),
    TREADMILL("treadmill", "Treadmill", Icons.Filled.SelfImprovement),
    CYCLE("cycle", "Cycle", Icons.AutoMirrored.Filled.DirectionsBike),
    SWIM("swim", "Swim", Icons.Filled.Pool),
    ROW("row", "Row", Icons.Filled.Rowing),
    HIKE("hike", "Hike", Icons.Filled.Hiking),
    ELLIPTICAL("elliptical", "Elliptical", Icons.Filled.FitnessCenter),
    YOGA("yoga", "Yoga", Icons.Filled.Spa),
    REST("rest", "Rest Day", Icons.Filled.Hotel),
    OTHER("other", "Other", Icons.Filled.MoreHoriz);

    val isRest: Boolean get() = this == REST

    companion object {
        fun fromCode(code: String): CardioType =
            entries.firstOrNull { it.code == code } ?: OTHER
    }
}
