package com.forge.app.ui.cardio

import com.forge.app.ui.common.clickableLabeled
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeChromeButton
import com.forge.app.ui.common.ForgeChromeIconButton
import com.forge.app.ui.common.GROUP_OUTER
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.memberFill

/*
 * Cardio's private pieces of the grouped-surface kit (`ui/common/ForgeGroups.kt`): the small filled
 * link capsule that replaced the accent `action →` links, the filled panel a chart sits on, a pager
 * arrow that can go passive, and a filled chip for pick-one sets too long for segments.
 */

/**
 * A quiet text link: mono, lowercase, on `onBg`, with an arrow when it navigates. It was a filled
 * capsule for a day (2026-09-27) and "Weeks" read as a button fighting the week it sits beside, the
 * same note Home's "view all" got, so both are text again. Touch comes from padding.
 */
@Composable
internal fun CardioLinkCapsule(
    text: String,
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    arrow: Boolean = true
) {
    Box(
        modifier
            .clickableLabeled(label, onClick = onClick)
            .heightIn(min = 48.dp)
            .padding(horizontal = 4.dp, vertical = 4.dp),
        contentAlignment = Alignment.CenterEnd
    ) {
        Text(
            text.lowercase().let { if (arrow) "$it →" else it },
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onBackground
        )
    }
}

/** A mono section anchor with an optional [CardioLinkCapsule] at its end. */
@Composable
internal fun CardioSectionHeader(
    label: String,
    modifier: Modifier = Modifier,
    action: String? = null,
    actionLabel: String? = null,
    onAction: (() -> Unit)? = null
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        EditorialHeader(label = label, muted = muted, accent = accent, modifier = Modifier.weight(1f))
        if (action != null && onAction != null) {
            CardioLinkCapsule(action, actionLabel ?: action, onAction)
        }
    }
}

/**
 * The frame a chart sits in. It was a filled rounded panel for a day (2026-09-27) and the week and
 * trend charts read as boxed-in widgets, so it is open again: charts sit on the page, and a panel
 * that opens something keeps its tap through [modifier]. [shape] clips the tap ripple only.
 */
@Composable
internal fun CardioChartPanel(
    modifier: Modifier = Modifier,
    shape: Shape = RoundedCornerShape(GROUP_OUTER),
    content: @Composable ColumnScope.() -> Unit
) {
    Column(
        modifier
            .fillMaxWidth()
            .clip(shape),
        content = content
    )
}

/** A glyph pager capsule. One that cannot move renders passive and says so, never a dead button. */
@Composable
internal fun CardioPagerButton(icon: ImageVector, label: String, enabled: Boolean, onClick: () -> Unit) {
    if (enabled) {
        ForgeChromeIconButton(icon, label, onClick)
    } else {
        Box(
            Modifier
                .minimumInteractiveComponentSize()
                .heightIn(min = 44.dp)
                .widthIn(min = 44.dp)
                .clip(RoundedCornerShape(50))
                .background(MaterialTheme.colorScheme.surfaceContainerHigh.copy(alpha = 0.35f))
                .semantics { contentDescription = label; disabled() },
            contentAlignment = Alignment.Center
        ) {
            Icon(
                icon,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f),
                modifier = Modifier.size(20.dp)
            )
        }
    }
}

/** A filled pick-one chip, for a set too long for [com.forge.app.ui.common.ForgeSlidingSegments]. */
@Composable
internal fun CardioFilledChip(text: String, selected: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(50)
    Box(
        Modifier
            .minimumInteractiveComponentSize()
            .memberFill(shape, selected)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; role = Role.RadioButton }
            .padding(horizontal = 14.dp, vertical = 9.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text,
            style = MaterialTheme.typography.bodyMedium,
            color = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}
