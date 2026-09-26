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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.outlined.Accessibility
import androidx.compose.material.icons.outlined.AddAPhoto
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
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
 * With no photos (or with the gallery locked) the tiles become empty picture frames of the same size,
 * labelled with the angles to shoot, and the invitation sits on the page beneath them.
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

/**
 * The zero (or locked) state: the same three tiles the photos will fill, drawn as empty frames, so
 * the section keeps its shape whether it holds photos or not. Each frame is labelled with the angle
 * it is waiting for, which doubles as the one tip worth giving: shoot front, side and back. The
 * first frame is the one that asks to be filled. The words sit on the page under the frames.
 */
@Composable
private fun GalleryInvite(locked: Boolean, muted: Color, onOpenGallery: () -> Unit) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        FRAME_POSES.forEachIndexed { i, pose ->
            EmptyFrame(
                label = pose,
                // Locked: one lock in the middle frame says it once; three would be a pattern.
                icon = when {
                    locked -> if (i == 1) Icons.Outlined.Lock else null
                    i == 0 -> Icons.Outlined.AddAPhoto
                    else -> Icons.Outlined.Accessibility
                },
                lead = i == 0 && !locked,
                description = if (locked) "Unlock progress photos" else "Add a $pose photo",
                onClick = onOpenGallery,
                modifier = Modifier.weight(1f)
            )
        }
    }
    Spacer(Modifier.height(12.dp))
    Text(
        if (locked) "Progress photos are locked" else "Add your first progress photo",
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.onBackground
    )
    Text(
        if (locked) "Tap to unlock" else "Same spot, same light, every few weeks.",
        style = MaterialTheme.typography.bodySmall,
        color = muted
    )
}

/** The angles the empty frames ask for, in the order a check-in is usually shot. */
private val FRAME_POSES = listOf("Front", "Side", "Back")

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
    EmptyFrame(
        label = "Add",
        icon = Icons.Outlined.AddAPhoto,
        lead = true,
        description = "Add a photo",
        onClick = onOpenGallery,
        modifier = modifier
    )
}

/**
 * An empty picture frame, the exact size and corner of a photo tile: a quiet fill inside a dashed
 * edge (the dash is what says "a slot", where a solid line would say "a box"), an icon in the
 * middle, and a label where a filled tile carries its date. The [lead] frame is the one to fill
 * next, so its edge and icon take the accent and it reads as the place to tap.
 */
@Composable
private fun EmptyFrame(
    label: String,
    icon: ImageVector?,
    lead: Boolean,
    description: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val outline = MaterialTheme.colorScheme.outline
    val accent = MaterialTheme.colorScheme.primary
    val edge = if (lead) accent.copy(alpha = 0.7f) else outline.copy(alpha = 0.6f)
    Box(
        modifier.aspectRatio(0.75f)
            .clip(TileShape)
            .background(if (lead) accent.copy(alpha = 0.15f) else outline.copy(alpha = 0.15f))
            .drawBehind {
                val stroke = 1.5.dp.toPx()
                val r = 12.dp.toPx()
                drawRoundRect(
                    color = edge,
                    topLeft = Offset(stroke / 2, stroke / 2),
                    size = Size(size.width - stroke, size.height - stroke),
                    cornerRadius = CornerRadius(r, r),
                    style = Stroke(width = stroke, pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 5.dp.toPx())))
                )
            }
            .bounceClick { onClick() }
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center
    ) {
        if (icon != null) {
            Icon(
                icon,
                contentDescription = null,
                tint = if (lead) accent else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(if (lead) 28.dp else 32.dp)
            )
        }
        Text(
            label,
            style = MaterialTheme.typography.bodySmall,
            color = if (lead) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.align(Alignment.BottomStart).padding(horizontal = 8.dp, vertical = 6.dp)
        )
    }
}

// ON THIS DAY was cut from the profile 2026-07-24: Home already renders the same throwback
// (OverviewScreen's OnThisDayCard), and a mark that only repeats another screen's answer is cut,
// not copied (§4.3). It was also the page's one prose-only section, hung off a decorative accent
// rule (§1: a line exists only as data).
