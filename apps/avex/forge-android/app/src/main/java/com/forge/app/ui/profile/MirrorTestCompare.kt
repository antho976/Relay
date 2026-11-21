package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.photo.musclesFromCodes
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.formatWeightDelta
import com.forge.app.ui.common.currentLocale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.text.SimpleDateFormat
import java.time.ZoneId
import java.util.Date
import kotlin.math.abs

private enum class CompareMode(val label: String) { SLIDER("Slider"), SPLIT("Split") }

/**
 * Full-screen comparison of [pair] (exactly two photos), ordered oldest→newest so "before" is always
 * the older shot. A Slider view overlays the two with a draggable reveal handle; a Split view sets
 * them side by side. The bottom readout carries the time apart, the weight change and the pose.
 */
@Composable
internal fun CompareSheet(
    pair: List<ProgressPhoto>,
    zone: ZoneId,
    weightUnit: WeightUnit,
    fileFor: (ProgressPhoto) -> File,
    onDismiss: () -> Unit
) {
    if (pair.size < 2) { onDismiss(); return }
    val ordered = remember(pair) { pair.sortedBy { it.takenAtMs } }
    val before = ordered[0]
    val after = ordered[1]
    var mode by remember { mutableStateOf(CompareMode.SLIDER) }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var sharing by remember { mutableStateOf(false) }
    // A render that produced nothing used to be swallowed by `uri?.let` — the icon simply came back
    // and the user was left guessing (P-09). The sheet is a Dialog, so the root snackbar host would
    // sit behind it; the line goes here, where the action was taken.
    var shareFailed by remember { mutableStateOf(false) }
    val accentArgb = accent.toArgb()

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        GalleryTheme {
        Column(Modifier.fillMaxSize().background(Color.Black)) {
            // Top row: close + Slider/Split toggle.
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                IconButton(onClick = onDismiss) { Icon(Icons.Filled.Close, contentDescription = "Close", tint = Color.White) }
                SingleChoiceSegmentedButtonRow {
                    CompareMode.entries.forEachIndexed { i, m ->
                        SegmentedButton(
                            selected = m == mode,
                            onClick = { mode = m },
                            shape = SegmentedButtonDefaults.itemShape(i, CompareMode.entries.size),
                            colors = SegmentedButtonDefaults.colors(
                                activeContainerColor = MaterialTheme.colorScheme.primaryContainer,
                                activeContentColor = Color.White,
                                inactiveContainerColor = Color.Transparent,
                                inactiveContentColor = Color.White,
                                activeBorderColor = Color.White.copy(alpha = 0.35f),
                                inactiveBorderColor = Color.White.copy(alpha = 0.35f)
                            ),
                            label = { Text(m.label) }
                        )
                    }
                }
                IconButton(onClick = {
                    if (sharing) return@IconButton
                    sharing = true
                    scope.launch {
                        val uri = withContext(Dispatchers.Default) {
                            val span = gallerySpanLabel(before.takenAtMs, after.takenAtMs, zone)
                            val bw = before.weightLb
                            val aw = after.weightLb
                            // Delta-only (GYMAP-55): the change, never the absolute bodyweight, on a public card.
                            val delta = if (bw != null && aw != null && abs(aw - bw) >= 0.1) {
                                val d = aw - bw
                                (if (d > 0) "+" else "−") + formatWeightDelta(abs(d), weightUnit)
                            } else null
                            val bp = PhotoPose.fromKey(before.pose)
                            val ap = PhotoPose.fromKey(after.pose)
                            val shared = musclesFromCodes(before.muscles.filter { it in after.muscles })
                            val poseLine = when {
                                shared.isNotEmpty() -> shared.joinToString(" · ") { it.displayName }.uppercase()
                                bp != null && bp == ap -> bp.label.uppercase()
                                bp != null && ap != null -> "${bp.label} → ${ap.label}".uppercase()
                                else -> null
                            }
                            BeforeAfterCardRenderer.render(
                                context, fileFor(before), fileFor(after),
                                before.takenAtMs, after.takenAtMs, span, delta, poseLine, accentArgb
                            )
                        }
                        sharing = false
                        shareFailed = uri == null
                        uri?.let { BeforeAfterCardRenderer.share(context, it) }
                    }
                }) {
                    Icon(Icons.Filled.Share, contentDescription = "Share", tint = if (sharing) muted else Color.White)
                }
            }

            if (shareFailed) {
                Text(
                    "Couldn't build the card. Try again.",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 2.dp)
                )
            }

            Box(Modifier.fillMaxWidth().weight(1f)) {
                when (mode) {
                    CompareMode.SLIDER -> SliderCompare(before, after, fileFor, accent)
                    CompareMode.SPLIT -> Row(Modifier.fillMaxSize()) {
                        ComparePane("Before", before, fileFor, accent, muted, Modifier.weight(1f))
                        Box(Modifier.fillMaxHeight().width(1.dp).background(Color.White.copy(alpha = 0.15f)))
                        ComparePane("After", after, fileFor, accent, muted, Modifier.weight(1f))
                    }
                }
            }

            CompareReadout(before, after, zone, weightUnit, onBg = Color.White, muted = muted)
        }
        }
    }
}

/** The slider view: [after] fills the frame, [before] is clipped to a draggable left fraction. */
@Composable
private fun SliderCompare(
    before: ProgressPhoto,
    after: ProgressPhoto,
    fileFor: (ProgressPhoto) -> File,
    accent: Color
) {
    var fraction by remember { mutableFloatStateOf(0.5f) }
    BoxWithConstraints(
        Modifier.fillMaxSize()
            .pointerInput(Unit) { detectTapGestures { fraction = (it.x / size.width).coerceIn(0f, 1f) } }
            .pointerInput(Unit) { detectDragGestures { change, _ -> fraction = (change.position.x / size.width).coerceIn(0f, 1f) } }
    ) {
        val w = maxWidth
        // The drag position is read only in draw and in the offset lambdas below, never here: read in
        // this scope it recomposed both images and the knob on every drag event.
        val afterFile = remember(after) { fileFor(after) }
        val beforeFile = remember(before) { fileFor(before) }
        GalleryFullImage(afterFile, Modifier.fillMaxSize())
        // Draw `before` at full size but clip its paint to the left fraction, so both stay aligned.
        GalleryFullImage(
            beforeFile,
            Modifier.fillMaxSize().drawWithContent {
                clipRect(right = size.width * fraction) { this@drawWithContent.drawContent() }
            }
        )
        // Divider + knob.
        Box(Modifier.align(Alignment.CenterStart).offset { IntOffset((w * fraction - 1.dp).roundToPx(), 0) }.fillMaxHeight().width(2.dp).background(Color.White.copy(alpha = 0.85f)))
        Box(
            Modifier.align(Alignment.CenterStart).offset { IntOffset((w * fraction - 20.dp).roundToPx(), 0) }.size(40.dp)
                .clip(CircleShape).background(Color.White),
            contentAlignment = Alignment.Center
        ) {
            Icon(Icons.Filled.SwapHoriz, contentDescription = null, tint = Color.Black)
        }
        // Corner tags.
        CornerTag("Before", Modifier.align(Alignment.TopStart))
        CornerTag("After", Modifier.align(Alignment.TopEnd))
    }
}

/** A small "Before" / "After" label on a dark pill, legible over any photo. */
@Composable
private fun CornerTag(text: String, modifier: Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.labelMedium,
        color = Color.White,
        modifier = modifier.padding(12.dp).clip(CircleShape)
            .background(Color.Black.copy(alpha = 0.6f)).padding(horizontal = 10.dp, vertical = 4.dp)
    )
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
        Text(tag, style = MaterialTheme.typography.titleSmall, color = Color.White)
        Text(
            SimpleDateFormat("MMM d, yyyy", currentLocale()).format(Date(photo.takenAtMs)),
            style = MaterialTheme.typography.bodySmall, color = muted, textAlign = TextAlign.Center
        )
        Spacer(Modifier.height(8.dp))
        GalleryFullImage(fileFor(photo), Modifier.fillMaxWidth().weight(1f))
    }
}

/** The bottom line under either compare view: time apart · weight change · pose. */
@Composable
private fun CompareReadout(
    before: ProgressPhoto,
    after: ProgressPhoto,
    zone: ZoneId,
    weightUnit: WeightUnit,
    onBg: Color,
    muted: Color
) {
    val span = gallerySpanLabel(before.takenAtMs, after.takenAtMs, zone)
    val apart = if (span.isEmpty()) "Same day" else "$span apart"
    val bw = before.weightLb
    val aw = after.weightLb
    val weightLine = if (bw != null && aw != null) {
        val d = aw - bw
        if (kotlin.math.abs(d) < 0.1) "${formatWeight(aw, weightUnit)} · no change"
        else "${formatWeight(bw, weightUnit)} → ${formatWeight(aw, weightUnit)} · ${formatWeightDelta(d, weightUnit)}"
    } else null
    val bp = PhotoPose.fromKey(before.pose)
    val ap = PhotoPose.fromKey(after.pose)
    val poseLine = when {
        bp != null && bp == ap -> bp.label
        bp != null && ap != null -> "${bp.label} → ${ap.label}"
        else -> null
    }
    // The muscles both shots agree on — what this comparison is actually evidence about. Only the
    // shared ones: a muscle tagged on one end alone says nothing about a change between them.
    val sharedMuscles = musclesFromCodes(before.muscles.filter { it in after.muscles })
        .takeIf { it.isNotEmpty() }?.joinToString(" · ") { it.displayName }
    val tagLine = listOfNotNull(poseLine, sharedMuscles)
        .takeIf { it.isNotEmpty() }?.joinToString(" · ")

    Column(
        Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 16.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(apart, style = MaterialTheme.typography.titleMedium, color = onBg)
        if (weightLine != null) {
            Spacer(Modifier.height(4.dp))
            Text(weightLine, style = MaterialTheme.typography.bodyMedium, color = muted)
        }
        if (tagLine != null) {
            Spacer(Modifier.height(4.dp))
            // §14: the accent carries marks, not meaning-bearing text — four of the five accent
            // choices fail AA as body text, so this line rides onBackground like its siblings.
            Text(tagLine, style = MaterialTheme.typography.bodySmall, color = onBg)
        }
    }
}
