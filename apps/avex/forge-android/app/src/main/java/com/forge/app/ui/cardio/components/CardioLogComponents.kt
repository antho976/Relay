package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.domain.units.filterDecimalInput
import com.forge.app.ui.common.Corners
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.memberFill
import com.forge.app.ui.common.memberShape

/*
 * The cardio log's input helpers: the glyph tile the custom-activity dialog picks with, and the
 * parsers and caps every typed field runs through.
 */

/** A pick-one glyph tile (the custom-activity icon grid): the glyph alone, on its own member. */
@Composable
internal fun CardioGlyphTile(
    icon: ImageVector,
    label: String,
    selected: Boolean,
    corners: Corners,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val shape = memberShape(corners, selected)
    Box(
        modifier
            .memberFill(shape, selected)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; role = Role.RadioButton; contentDescription = label }
            .padding(vertical = 14.dp),
        contentAlignment = Alignment.Center
    ) {
        Icon(
            icon,
            contentDescription = null,
            tint = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(22.dp)
        )
    }
}

/** A bare number for seeding an editable field — drops a whole value's trailing ".0" (e.g. incline). */
internal fun plainDecimalInput(v: Double): String =
    if (v % 1.0 == 0.0) v.toInt().toString() else v.toString()

internal fun sanitizeDecimal(input: String): String = filterDecimalInput(input).take(6)

/**
 * The duration field's text, held to one day. The field allows five characters, so a typo like
 * "99999" (or "99:99") would otherwise be stored as-is and inflate the week's totals, the streak,
 * the records and the cardio goals. The bound is the importers' (a longer single entry is a slip).
 */
internal fun capDurationText(input: String): String =
    if (parseDurationMin(input) > com.forge.app.data.importer.ImportBounds.MAX_CARDIO_MINUTES) {
        com.forge.app.data.importer.ImportBounds.MAX_CARDIO_MINUTES.toString()
    } else input

/**
 * The distance field's text (in the display unit), held to the importers' single-entry bound so the
 * field shows what will be saved rather than a "1500" that is silently stored as 1000 km. The miles
 * bound is rounded DOWN to one decimal so the capped text itself never parses past the bound.
 */
internal fun capDistanceText(input: String, useMiles: Boolean): String {
    val maxKm = com.forge.app.data.importer.ImportBounds.MAX_CARDIO_DISTANCE_KM
    val km = com.forge.app.domain.units.parseToKm(input, useMiles) ?: return input
    if (km <= maxKm) return input
    val maxDisplay = kotlin.math.floor(com.forge.app.domain.units.toDisplayDistance(maxKm, useMiles) * 10.0) / 10.0
    return plainDecimalInput(maxDisplay)
}

/** Keep digits and a single colon, capped at "HH:MM" width, for the duration field (GYMAP-41). */
internal fun sanitizeDuration(input: String): String {
    val filtered = input.filter { it.isDigit() || it == ':' }
    val firstColon = filtered.indexOf(':')
    val collapsed = if (firstColon == -1) filtered
    else filtered.substring(0, firstColon + 1) + filtered.substring(firstColon + 1).replace(":", "")
    return collapsed.take(5)
}

/**
 * Parse the duration field into whole minutes (GYMAP-41). Accepts either a plain minute count
 * ("90" -> 90) or an H:MM clock value ("1:30" -> 90); a lone number stays minutes so existing
 * plain-number entry is unchanged. The store has no seconds column, so no sub-minute component is
 * read; malformed parts count as 0.
 */
internal fun parseDurationMin(input: String): Int {
    val t = input.trim()
    if (t.isEmpty()) return 0
    val colon = t.indexOf(':')
    if (colon < 0) return t.toIntOrNull() ?: 0
    val hours = t.substring(0, colon).toIntOrNull() ?: 0
    val minutes = t.substring(colon + 1).toIntOrNull() ?: 0
    return hours * 60 + minutes
}
