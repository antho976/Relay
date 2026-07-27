package com.forge.app.ui.profile

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.EditorialLegend

/**
 * The profile's two local wrappers over the shared `ui/common` editorial primitives — a section
 * anchor and a chart legend. The figures themselves are the shared [com.forge.app.ui.common.EditorialFigure]
 * (§8: reuse, never re-implement); the profile's own 36sp `StatCell` copy of it was retired
 * 2026-07-24. Bordered "grey box" cards stay gone on purpose — the cover photo up top dissolves
 * into the background, so everything below it sits on the same open page instead of fighting it.
 */

/**
 * A quiet small-caps section anchor (label + optional action) — the shared [EditorialHeader]
 * voice (mono labelLarge 13sp) with the profile's local inset kept. The action reads accent
 * mono per §11 (actions are accent, labels are muted).
 */
@Composable
internal fun SectionHeader(
    label: String,
    muted: Color,
    action: String? = null,
    onAction: (() -> Unit)? = null
) {
    EditorialHeader(
        label = label,
        muted = muted,
        accent = MaterialTheme.colorScheme.primary,
        // §7 rhythm: 28 above the header, 10 below it — the air IS the separator.
        //
        // NO start padding. It carried `start = 2.dp`, which put every section anchor 2dp right of
        // the rows it heads — measured on device as a visible ~4px step between "BODY" and "WEIGHT".
        // The page has ONE left edge (§7's 24dp gutter); an anchor that sits off it reads as a
        // misprint, and optical-centring a mono capital against mono capitals corrects nothing.
        modifier = Modifier.padding(bottom = 10.dp),
        action = action,
        onAction = onAction
    )
}

/** A dot-plus-caption legend line for the open full-width charts — the shared [EditorialLegend]. */
@Composable
internal fun ChartCaption(color: Color, label: String, muted: Color) {
    EditorialLegend(color, label, muted)
}
