package com.forge.app.domain.cardio

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.DirectionsBike
import androidx.compose.material.icons.automirrored.filled.DirectionsRun
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.FitnessCenter
import androidx.compose.material.icons.filled.Hiking
import androidx.compose.material.icons.filled.MoreHoriz
import androidx.compose.material.icons.filled.Pool
import androidx.compose.material.icons.filled.SelfImprovement
import androidx.compose.ui.graphics.vector.ImageVector

/**
 * The curated glyph set a user picks from when creating a custom cardio activity (GYMAP-37). It reuses
 * the exact same icons the built-in [CardioType]s draw, so a custom "Padel" sits in the picker/rows
 * visually indistinguishable from a stock type — no second icon language.
 *
 * Each entry has a STABLE [key] that is persisted on the custom type (never the icon itself, which is
 * a build-specific object). [icon] resolves a key back to its vector; an unknown/removed key falls
 * back to the neutral "other" glyph rather than crashing.
 */
object CardioGlyphs {

    data class Glyph(val key: String, val icon: ImageVector)

    /** Ordered for the picker grid. Keys name the glyph, not any one activity, so reusing "run" for a
     *  custom "Trail run" reads naturally. */
    val catalog: List<Glyph> = listOf(
        Glyph("run", Icons.AutoMirrored.Filled.DirectionsRun),
        Glyph("walk", CardioIcons.Walk),
        Glyph("treadmill", CardioIcons.Treadmill),
        Glyph("cycle", Icons.AutoMirrored.Filled.DirectionsBike),
        Glyph("swim", Icons.Filled.Pool),
        Glyph("row", CardioIcons.RowingMachine),
        Glyph("hike", Icons.Filled.Hiking),
        Glyph("gym", Icons.Filled.FitnessCenter),
        Glyph("bolt", Icons.Filled.Bolt),
        Glyph("yoga", Icons.Filled.SelfImprovement),
        Glyph("other", Icons.Filled.MoreHoriz),
    )

    /** The glyph a custom type gets when none is chosen / a stored key no longer exists. */
    const val DEFAULT_KEY = "other"

    fun icon(key: String): ImageVector =
        catalog.firstOrNull { it.key == key }?.icon
            ?: catalog.first { it.key == DEFAULT_KEY }.icon
}
