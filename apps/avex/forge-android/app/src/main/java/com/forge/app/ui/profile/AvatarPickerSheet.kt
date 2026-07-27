package com.forge.app.ui.profile

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
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
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.bounceClick
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The profile-photo picker (GYMAP-22) — a modal sheet reached by tapping the identity cover. The
 * do-it-now action ("Select your own") sits at the top and hands off to the system Photo Picker;
 * below it, the app's bundled covers are laid out in labelled categories. Tapping a cover bakes it
 * into the avatar (see [DefaultAvatars]); the active default gets an accent ring.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun AvatarPickerSheet(
    selectedKey: String?,
    onPickOwn: () -> Unit,
    onSelectDefault: (DefaultAvatars.Item) -> Unit,
    onDismiss: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    // A grid of 21 covers wants the room — open fully rather than at a half-height stop.
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        // §5: a modal is a `surface` fill — M3 defaults to the unthemed `surfaceContainerLow`.
        containerColor = MaterialTheme.colorScheme.surface
    ) {
        val gridState = rememberLazyGridState()
        // Decode each cover once per sheet open — the grid disposes off-screen thumbs, so without this
        // scrolling a cover out and back would re-run BitmapFactory (and flash the placeholder). ~21
        // downsampled (≤400px) thumbs is a few MB, kept for the sheet's lifetime only.
        val thumbCache = remember { mutableMapOf<Int, ImageBitmap>() }
        LazyVerticalGrid(
            state = gridState,
            columns = GridCells.Fixed(3),
            // Soft-fade the scrolling grid into the sheet at whichever edge still has content, so it
            // dissolves instead of ending on a hard cut (matches the profile cover's edge fade). An
            // offscreen DstIn mask fades only this layer's pixels to transparent = the sheet surface
            // behind it, so it needs no knowledge of the surface colour; gated on scroll so the title
            // stays crisp at rest and the last row isn't dimmed once fully scrolled.
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
                            1f to (if (gridState.canScrollForward) Color.Transparent else Color.Black)
                        ),
                        blendMode = BlendMode.DstIn
                    )
                },
            contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 32.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            item(span = { GridItemSpan(maxLineSpan) }) {
                Column {
                    Text("Profile photo", style = MaterialTheme.typography.headlineSmall, color = onBg)
                    Spacer(Modifier.height(12.dp))
                    ForgePrimaryCapsule(
                        label = "Select your own",
                        onClick = onPickOwn,
                        modifier = Modifier.fillMaxWidth()
                    )
                    Spacer(Modifier.height(14.dp))
                    Text(
                        "Or choose one of ours",
                        style = MaterialTheme.typography.bodySmall,
                        color = muted, fontStyle = FontStyle.Italic
                    )
                }
            }
            DefaultAvatars.categories.forEach { category ->
                item(span = { GridItemSpan(maxLineSpan) }) {
                    EditorialHeader(category.label, muted, accent, Modifier.padding(top = 14.dp, bottom = 4.dp))
                }
                itemsIndexed(category.items, key = { _, it -> it.key }) { index, avatar ->
                    DefaultCoverThumb(
                        avatar = avatar,
                        label = "${category.label} ${index + 1}",
                        selected = avatar.key == selectedKey,
                        accent = accent,
                        cache = thumbCache,
                        onClick = { onSelectDefault(avatar) }
                    )
                }
            }
        }
    }
}

/** One tappable cover thumbnail — decoded downsampled off the main thread; ringed when it's active. */
@Composable
private fun DefaultCoverThumb(
    avatar: DefaultAvatars.Item,
    label: String,
    selected: Boolean,
    accent: androidx.compose.ui.graphics.Color,
    cache: MutableMap<Int, ImageBitmap>,
    onClick: () -> Unit
) {
    val context = LocalContext.current
    val shape = RoundedCornerShape(16.dp)
    // Thumbs only need ~grid-cell resolution — sample down (source ≤1600px → ≤400px) to keep the
    // open picker's bitmap footprint small; decode off the main thread so scrolling stays smooth.
    // Seed from the sheet-scoped [cache] so a cover scrolled back into view shows instantly and never
    // re-decodes (the producer early-returns when the value is already present).
    val bitmap by produceState<ImageBitmap?>(cache[avatar.resId], avatar.resId) {
        if (value != null) return@produceState
        val decoded = withContext(Dispatchers.IO) {
            runCatching {
                BitmapFactory.decodeResource(
                    context.resources, avatar.resId,
                    BitmapFactory.Options().apply { inSampleSize = 4; inScaled = false }
                )?.asImageBitmap()
            }.getOrNull()
        }
        if (decoded != null) cache[avatar.resId] = decoded
        value = decoded
    }
    Box(
        Modifier
            .fillMaxWidth()
            .aspectRatio(1f)
            .semantics { contentDescription = if (selected) "$label, selected" else label }
    ) {
        Box(Modifier.fillMaxSize().clip(shape).bounceClick { onClick() }) {
            bitmap?.let {
                Image(bitmap = it, contentDescription = null, modifier = Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            } ?: Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surfaceVariant))
        }
        // Ring drawn ABOVE the cropped image so the accent border is never overdrawn.
        if (selected) Box(Modifier.fillMaxSize().border(2.dp, accent, shape))
    }
}
