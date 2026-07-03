package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.bounceClick
import java.io.File
import java.text.SimpleDateFormat
import java.time.ZoneId
import java.util.Date
import java.util.Locale

/**
 * The sticky bar shown at the bottom while compare mode is on: it counts the selection (max 2) and
 * exposes the Compare action once two photos are picked.
 */
@Composable
internal fun CompareBar(
    selectedCount: Int,
    onClear: () -> Unit,
    onCompare: () -> Unit,
    muted: Color,
    accent: Color
) {
    Surface(color = MaterialTheme.colorScheme.surface, tonalElevation = 3.dp) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                when (selectedCount) {
                    0 -> "Tap two photos to compare"
                    1 -> "Pick one more…"
                    else -> "2 selected"
                },
                style = MaterialTheme.typography.bodyMedium, color = muted
            )
            Spacer(Modifier.width(16.dp))
            Spacer(Modifier.weight(1f))
            if (selectedCount > 0) {
                Text(
                    "Clear", style = MaterialTheme.typography.labelMedium, color = accent,
                    modifier = Modifier.bounceClick { onClear() }.padding(horizontal = 8.dp, vertical = 6.dp)
                )
                Spacer(Modifier.width(8.dp))
            }
            Button(enabled = selectedCount == 2, onClick = onCompare) { Text("Compare") }
        }
    }
}

/**
 * Full-screen before/after comparison of [pair] (exactly two photos). They're ordered oldest→newest
 * so the left pane is always "before", and the gap between their dates is shown at the bottom.
 */
@Composable
internal fun CompareSheet(
    pair: List<ProgressPhoto>,
    fileFor: (ProgressPhoto) -> File,
    onDismiss: () -> Unit
) {
    if (pair.size < 2) { onDismiss(); return }
    val ordered = remember(pair) { pair.sortedBy { it.takenAtMs } }
    val before = ordered[0]
    val after = ordered[1]
    val zone = remember { ZoneId.systemDefault() }
    val apart = remember(before, after) {
        val span = gallerySpanLabel(before.takenAtMs, after.takenAtMs, zone)
        if (span.isEmpty()) "Same day" else "$span apart"
    }
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.94f))) {
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                IconButton(onClick = onDismiss) {
                    Icon(Icons.Filled.Close, contentDescription = "Close", tint = Color.White)
                }
                Text("Before / After", style = MaterialTheme.typography.labelMedium, color = Color.White.copy(alpha = 0.85f))
                Spacer(Modifier.width(48.dp))
            }

            Row(Modifier.fillMaxWidth().weight(1f)) {
                ComparePane("BEFORE", before, fileFor, accent, muted, Modifier.weight(1f))
                Box(Modifier.fillMaxHeight().width(1.dp).background(Color.White.copy(alpha = 0.15f)))
                ComparePane("AFTER", after, fileFor, accent, muted, Modifier.weight(1f))
            }

            Box(Modifier.fillMaxWidth().padding(vertical = 16.dp), contentAlignment = Alignment.Center) {
                Text(apart, style = MaterialTheme.typography.titleMedium, color = Color.White)
            }
        }
    }
}

@Composable
private fun ComparePane(
    tag: String,
    photo: ProgressPhoto,
    fileFor: (ProgressPhoto) -> File,
    accent: Color,
    muted: Color,
    modifier: Modifier = Modifier
) {
    Column(modifier.padding(horizontal = 6.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Text(tag, style = MaterialTheme.typography.labelMedium, color = accent)
        Text(
            SimpleDateFormat("MMM d, yyyy", Locale.getDefault()).format(Date(photo.takenAtMs)),
            style = MaterialTheme.typography.labelSmall, color = muted, textAlign = TextAlign.Center
        )
        Spacer(Modifier.height(8.dp))
        GalleryFullImage(fileFor(photo), Modifier.fillMaxWidth().weight(1f))
    }
}
