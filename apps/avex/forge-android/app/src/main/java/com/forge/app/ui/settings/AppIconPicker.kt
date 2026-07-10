@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.res.colorResource
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.R
import com.forge.app.appicon.AppIcon
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.bounceClick

/**
 * The Appearance-page "App icon" control — a single tappable row that shows the icon you're on now
 * and opens [AppIconPickerSheet] to change it, the same shape as the profile avatar picker. Settings
 * archetype: navigation-in-place, no inline grid crowding the page (DESIGN §3).
 */
@Composable
internal fun AppIconRow(currentKey: String, onOpen: () -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val current = AppIcon.fromKey(currentKey)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .bounceClick(onClick = onOpen)
            .padding(horizontal = 24.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(current.displayName, style = MaterialTheme.typography.bodyMedium, color = onBg)
            Text(
                "Tap to change your home-screen icon",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 10.sp,
            )
        }
        // The icon you have now — the tap target that opens the picker (whole row is tappable).
        AppIconThumb(current, isSelected = false, modifier = Modifier.size(44.dp))
    }
}

/**
 * The icon picker — a modal sheet of every launcher icon grouped under mono family headers, the
 * ringed one being the current pick. Mirrors [com.forge.app.ui.profile.AvatarPickerSheet].
 */
@Composable
internal fun AppIconPickerSheet(
    selectedKey: String,
    onSelect: (AppIcon) -> Unit,
    onDismiss: () -> Unit,
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val current = AppIcon.fromKey(selectedKey)
    // 30 icons want the room — open fully rather than at a half-height stop.
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val byFamily = AppIcon.entries.groupBy { it.family }

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheetState) {
        val gridState = rememberLazyGridState()
        LazyVerticalGrid(
            state = gridState,
            columns = GridCells.Fixed(4),
            // Soft-fade the scrolling grid into the sheet at whichever edge still has content (matches
            // the avatar sheet): an offscreen DstIn mask fades only this layer's pixels to the surface.
            modifier = Modifier
                .fillMaxWidth()
                .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
                .drawWithContent {
                    drawContent()
                    drawRect(
                        brush = Brush.verticalGradient(
                            0f to (if (gridState.canScrollBackward) Color.Transparent else Color.Black),
                            0.05f to Color.Black,
                            0.94f to Color.Black,
                            1f to (if (gridState.canScrollForward) Color.Transparent else Color.Black),
                        ),
                        blendMode = BlendMode.DstIn,
                    )
                },
            contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 32.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            item(span = { GridItemSpan(maxLineSpan) }) {
                Column {
                    Text("App icon", style = MaterialTheme.typography.headlineSmall, color = onBg)
                    Spacer(Modifier.height(4.dp))
                    Text(
                        "Changes your home-screen icon; it updates after a moment.",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted, fontStyle = FontStyle.Italic,
                    )
                }
            }
            AppIcon.families.forEach { family ->
                item(span = { GridItemSpan(maxLineSpan) }) {
                    EditorialHeader(family.name, muted, accent, Modifier.padding(top = 12.dp, bottom = 2.dp))
                }
                items(byFamily.getValue(family), key = { it.name }) { icon ->
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        AppIconThumb(
                            icon = icon,
                            isSelected = icon == current,
                            modifier = Modifier.fillMaxWidth().aspectRatio(1f),
                            onClick = { onSelect(icon) },
                        )
                        Text(
                            icon.label,
                            style = MaterialTheme.typography.labelSmall,
                            color = if (icon == current) onBg else muted.copy(alpha = 0.7f),
                            fontSize = 9.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            textAlign = TextAlign.Center,
                        )
                    }
                }
            }
        }
    }
}

/** One rounded launcher-icon thumbnail — full-bleed art (the default composites its emblem over the
 *  real launcher background), an accent ring when it's the active pick, bounce when tappable. */
@Composable
private fun AppIconThumb(
    icon: AppIcon,
    isSelected: Boolean,
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
) {
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    val shape = RoundedCornerShape(16.dp)
    val label = "${icon.family} ${icon.label} icon" + if (isSelected) ", selected" else ""
    Box(
        contentAlignment = Alignment.Center,
        modifier = modifier
            .clip(shape)
            .then(if (onClick != null) Modifier.bounceClick(onClick = onClick) else Modifier)
            .background(if (icon.isDefault) colorResource(R.color.ic_launcher_background) else Color.Transparent)
            .border(
                width = if (isSelected) 2.dp else 1.dp,
                color = if (isSelected) accent else outline.copy(alpha = 0.35f),
                shape = shape,
            )
            .semantics { contentDescription = label },
    ) {
        Image(
            painter = painterResource(icon.previewRes),
            contentDescription = null,
            modifier = Modifier.fillMaxSize(),
        )
    }
}
