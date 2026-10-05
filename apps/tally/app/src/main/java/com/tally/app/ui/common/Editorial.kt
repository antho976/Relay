package com.tally.app.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.tally.app.ui.theme.MonoAction
import com.tally.app.ui.theme.MonoSectionAnchor

/** The page gutter, from Avex. */
val GUTTER = 24.dp

/**
 * The widest one column of reading grows, gutters included. On a tablet a phone column stretched
 * to the window puts a label a screen away from its reading; past this width a single column
 * stops and centres instead.
 */
val READING_WIDTH = 640.dp

/**
 * A screen names itself with this serif title and nothing else (never a top-bar title). The
 * [context] line sits UNDER it in sans, Avex's Academy voice: no eyebrow above a heading.
 */
@Composable
fun PageTitle(title: String, modifier: Modifier = Modifier, context: String? = null) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(
            title,
            style = MaterialTheme.typography.headlineLarge,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.semantics { heading() },
        )
        if (context != null) {
            Text(context, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/**
 * The mono section anchor, with a reading or a `view all →` action at its end. Air and this
 * header are the only separator between sections: never a rule.
 */
@Composable
fun SectionHeader(
    label: String,
    modifier: Modifier = Modifier,
    meta: String? = null,
    action: String? = null,
    onAction: (() -> Unit)? = null,
) {
    Row(
        modifier.fillMaxWidth().heightIn(min = 32.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            label.uppercase(),
            style = MonoSectionAnchor,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f, fill = false).semantics { heading() },
        )
        when {
            action != null && onAction != null -> TextAction(action, onAction)
            meta != null -> Text(meta, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** Navigation as a mono `action →`, Avex level ③. Padded to a 48dp target. */
@Composable
fun TextAction(label: String, onClick: () -> Unit, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.onBackground) {
    Box(
        modifier
            .minimumInteractiveComponentSize()
            .bounceClick(label = label, role = Role.Button, onClick = onClick)
            .padding(horizontal = 4.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text("$label →", style = MaterialTheme.typography.labelLarge, color = color)
    }
}

/** One open figure: a serif number on the page with its mono label under it. No card. */
@Composable
fun Figure(
    value: String,
    label: String,
    modifier: Modifier = Modifier,
    valueColor: Color = MaterialTheme.colorScheme.onBackground,
    large: Boolean = false,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(
            value,
            style = if (large) MaterialTheme.typography.displayMedium else MaterialTheme.typography.headlineLarge,
            color = valueColor,
        )
        Text(label.uppercase(), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** The quiet italic aside: a zero-state line, a one-line explanation. Never a banner. */
@Composable
fun Aside(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        modifier = modifier,
        style = MaterialTheme.typography.bodyMedium.copy(fontStyle = FontStyle.Italic),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

/** One muted caption under a mark, at most one per section. */
@Composable
fun Caption(text: String, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.onSurfaceVariant) {
    Text(text, modifier = modifier, style = MaterialTheme.typography.bodySmall, color = color)
}

/**
 * The hero action: a page's one do-it-now act at full weight. Accent fill, 12dp corners, bold
 * mono label, ≥56dp from a minimum so it grows at 200% font.
 */
@Composable
fun HeroAction(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    Box(
        modifier
            .heightIn(min = 56.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.primary.copy(alpha = if (enabled) 1f else 0.35f))
            // On the accent fill an accent ring would vanish; it takes the label's colour instead.
            .bounceClick(
                enabled = enabled,
                label = text,
                role = Role.Button,
                focusColor = MaterialTheme.colorScheme.onPrimary,
                onClick = onClick,
            )
            .padding(horizontal = 20.dp, vertical = 16.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = MonoAction, color = MaterialTheme.colorScheme.onPrimary, textAlign = TextAlign.Center)
    }
}

/** The hero's sidekick: the same shape, filled with the raised surface, mono label. */
@Composable
fun SecondaryAction(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    destructive: Boolean = false,
    enabled: Boolean = true,
) {
    Box(
        modifier
            .heightIn(min = 56.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .bounceClick(enabled = enabled, label = text, role = Role.Button, onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 16.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text,
            style = MonoAction,
            color = (if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onBackground)
                .copy(alpha = if (enabled) 1f else 0.35f),
            textAlign = TextAlign.Center,
        )
    }
}

/** The round filled chrome capsule (back, settings, add), 44dp drawn, 48dp touch. */
@Composable
fun ChromeButton(icon: ImageVector, label: String, onClick: () -> Unit, modifier: Modifier = Modifier, tint: Color = MaterialTheme.colorScheme.onBackground) {
    Box(
        modifier
            .minimumInteractiveComponentSize()
            .size(44.dp)
            .clip(RoundedCornerShape(50))
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .bounceClick(label = label, role = Role.Button, focusShape = RoundedCornerShape(50), onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = label, tint = tint, modifier = Modifier.size(22.dp))
    }
}

/**
 * The top bar: a back capsule at the start, chrome capsules at the end, on the page itself (no
 * bar fill), inset below the status bar. It never names the screen.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TopBar(onBack: (() -> Unit)?, modifier: Modifier = Modifier, actions: @Composable RowScope.() -> Unit = {}) {
    Row(
        modifier
            .fillMaxWidth()
            .windowInsetsPadding(TopAppBarDefaults.windowInsets)
            .padding(horizontal = 16.dp, vertical = 8.dp)
            .heightIn(min = 48.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (onBack != null) ChromeButton(Icons.AutoMirrored.Rounded.ArrowBack, "Back", onBack)
        Box(Modifier.weight(1f))
        actions()
    }
}

/** A compact outlined pill drawn at a row's end; the ROW is the tap, never the pill. */
@Composable
fun RowPill(label: String, modifier: Modifier = Modifier) {
    Box(
        modifier
            .widthIn(min = 48.dp)
            .clip(RoundedCornerShape(50))
            .background(MaterialTheme.colorScheme.surfaceContainerHighest)
            .padding(horizontal = 14.dp, vertical = 7.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onBackground)
    }
}
