package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.outlined.AddPhotoAlternate
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.PhotoCamera
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.theme.MonoSectionAnchor
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date

/** How many photos the Profile previews. The last tile says how many more the Gallery holds. */
private const val PREVIEW_COUNT = 3

/**
 * GALLERY: the Profile's window onto the progress photos.
 *
 * A header row (the page's own section label, the photo count, a chevron) over three portrait
 * tiles, newest first, filling the page's width. When there are more photos than tiles, the last
 * tile says how many more. Every part of it, header and tiles alike, opens the Gallery, where
 * viewing, adding, comparing and albums all live; nothing here behaves differently from its
 * neighbour.
 *
 * With no photos (or with the gallery locked) the tiles are replaced by one tappable row that says
 * what to do, rather than a strip of empty frames that looked like broken images.
 */
@Composable
internal fun GalleryStrip(
    photos: List<ProgressPhoto>,
    fileFor: (ProgressPhoto) -> File,
    onOpenGallery: () -> Unit,
    muted: Color,
    locked: Boolean = false,
) {
    // A locked gallery shows nothing derived from the photos, not even how many there are.
    val shown = if (locked) emptyList() else photos
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 48.dp).bounceClick { onOpenGallery() },
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                "GALLERY",
                style = MonoSectionAnchor,
                color = muted,
                modifier = Modifier.weight(1f).semantics { heading() }
            )
            if (shown.isNotEmpty()) {
                Text(
                    "${shown.size} photo${if (shown.size == 1) "" else "s"}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = muted
                )
                Spacer(Modifier.width(4.dp))
            }
            Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = "Open gallery", tint = muted)
        }
        Spacer(Modifier.height(8.dp))
        if (shown.isEmpty()) {
            GalleryInvite(locked, muted, onOpenGallery)
        } else {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                val preview = shown.take(PREVIEW_COUNT)
                preview.forEachIndexed { i, photo ->
                    val more = if (i == preview.lastIndex) shown.size - preview.size else 0
                    PreviewTile(photo, fileFor(photo), more, onOpenGallery, Modifier.weight(1f))
                }
                // One or two photos: the next slot is where the next one goes.
                if (preview.size < PREVIEW_COUNT) {
                    AddTile(onOpenGallery, Modifier.weight(1f))
                    repeat(PREVIEW_COUNT - preview.size - 1) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
    }
}

/** The zero (or locked) state: one row that says what tapping it does. */
@Composable
private fun GalleryInvite(locked: Boolean, muted: Color, onOpenGallery: () -> Unit) {
    Row(
        Modifier.fillMaxWidth()
            .clip(TileShape)
            .background(MaterialTheme.colorScheme.outline.copy(alpha = 0.15f))
            .bounceClick { onOpenGallery() }
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            Modifier.size(48.dp).clip(CircleShape).background(MaterialTheme.colorScheme.outline.copy(alpha = 0.25f)),
            contentAlignment = Alignment.Center
        ) {
            Icon(
                if (locked) Icons.Outlined.Lock else Icons.Outlined.PhotoCamera,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onBackground
            )
        }
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(
                if (locked) "Progress photos are locked" else "Add your first progress photo",
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onBackground
            )
            Text(
                if (locked) "Tap to unlock" else "Same pose, same light, every few weeks.",
                style = MaterialTheme.typography.bodySmall,
                color = muted
            )
        }
    }
}

/** Preview tiles share one corner: smaller than the page's cards, because the tiles are smaller. */
private val TileShape = RoundedCornerShape(12.dp)

/** One preview photo: a portrait crop with its date, or a "+N" veil on the last tile. */
@Composable
private fun PreviewTile(
    photo: ProgressPhoto,
    file: File,
    more: Int,
    onOpenGallery: () -> Unit,
    modifier: Modifier = Modifier
) {
    val locale = currentLocale()
    val date = remember(photo.takenAtMs, locale) { SimpleDateFormat("MMM d", locale).format(Date(photo.takenAtMs)) }
    Box(
        modifier.aspectRatio(0.75f)
            .clip(TileShape)
            .bounceClick { onOpenGallery() }
            .semantics { contentDescription = if (more > 0) "Photo from $date, and $more more" else "Photo from $date" }
    ) {
        ProgressPhotoImage(file, Modifier.fillMaxSize(), reqPx = 420)
        if (more > 0) {
            Box(Modifier.matchParentSize().background(Color.Black.copy(alpha = 0.6f)), contentAlignment = Alignment.Center) {
                Text("+$more", style = MaterialTheme.typography.titleLarge, color = Color.White)
            }
        } else {
            Box(
                Modifier.matchParentSize().background(
                    Brush.verticalGradient(0.6f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.6f))
                )
            )
            Text(
                date,
                style = MaterialTheme.typography.bodySmall,
                color = Color.White,
                modifier = Modifier.align(Alignment.BottomStart).padding(horizontal = 8.dp, vertical = 6.dp)
            )
        }
    }
}

/** The empty slot after the last photo, while there are fewer photos than tiles. */
@Composable
private fun AddTile(onOpenGallery: () -> Unit, modifier: Modifier = Modifier) {
    Box(
        modifier.aspectRatio(0.75f)
            .clip(TileShape)
            .background(MaterialTheme.colorScheme.outline.copy(alpha = 0.15f))
            .bounceClick { onOpenGallery() }
            .semantics { contentDescription = "Add a photo" },
        contentAlignment = Alignment.Center
    ) {
        Icon(Icons.Outlined.AddPhotoAlternate, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

// ON THIS DAY was cut from the profile 2026-07-24: Home already renders the same throwback
// (OverviewScreen's OnThisDayCard), and a mark that only repeats another screen's answer is cut,
// not copied (§4.3). It was also the page's one prose-only section, hung off a decorative accent
// rule (§1: a line exists only as data).
