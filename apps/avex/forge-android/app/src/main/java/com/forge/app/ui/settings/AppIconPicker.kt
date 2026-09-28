@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
package com.forge.app.ui.settings

import androidx.compose.foundation.Image
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
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
import com.forge.app.ui.common.window.ModalBottomSheet
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.forge.app.appicon.AppIcon
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material3.Icon

/**
 * The Appearance page's App icon row: the icon you are on now as the row's tile, its name, and a
 * chevron into [AppIconPickerSheet].
 */
@Composable
internal fun AppIconRow(currentKey: String, onOpen: () -> Unit) {
    val current = AppIcon.fromKey(currentKey)
    SettingsRowContainer(interaction = Modifier.clickableLabeled("App icon, ${current.displayName}. Change", onClick = onOpen)) {
        AppIconThumb(current, isSelected = false, modifier = Modifier.size(40.dp), corner = 12.dp)
        SettingsRowText("App icon", current.displayName)
        Icon(
            Icons.AutoMirrored.Rounded.KeyboardArrowRight,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant
        )
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
    // Two dozen icons want the room — open fully rather than at a half-height stop.
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val byFamily = AppIcon.entries.groupBy { it.family }

    // §5: a modal is `containerColor = surface`. Left unset it took M3's stock surfaceContainerLow,
    // a purple-leaning grey belonging to no theme here (`design/AUDIT.md`, 2026-07-25).
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface
    ) {
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
                        "Your home screen updates a moment after you pick.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = muted,
                    )
                }
            }
            AppIcon.families.forEach { family ->
                item(span = { GridItemSpan(maxLineSpan) }) {
                    Text(
                        family.name.lowercase().replaceFirstChar { it.uppercase() },
                        style = MaterialTheme.typography.titleSmall,
                        color = muted,
                        modifier = Modifier.padding(top = 16.dp, bottom = 2.dp)
                    )
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
                            color = if (icon == current) onBg else muted,
                            overflow = TextOverflow.Ellipsis,
                            textAlign = TextAlign.Center,
                        )
                    }
                }
            }
        }
    }
}

/** One rounded launcher-icon thumbnail — the icon as a launcher shows it, an accent ring when it's
 *  the active pick, bounce when tappable. */
@Composable
private fun AppIconThumb(
    icon: AppIcon,
    isSelected: Boolean,
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    corner: androidx.compose.ui.unit.Dp = 16.dp,
) {
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline
    val shape = RoundedCornerShape(corner)
    val label = "${icon.family} ${icon.label} icon" + if (isSelected) ", selected" else ""
    Box(
        contentAlignment = Alignment.Center,
        modifier = modifier
            .clip(shape)
            .then(if (onClick != null) Modifier.bounceClick(onClick = onClick) else Modifier)
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
